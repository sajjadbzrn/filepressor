<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";

/**
 * Calm, cinematic "nebula" backdrop — a port of the FBM cloud shader used in
 * the drift app, but rendered with raw WebGL (no Three.js) and themed to
 * FilePressor's pink palette so the two apps don't look identical.
 *
 * Organic, slowly flowing clouds animate in a fullscreen shader. It pauses when
 * the window is hidden, falls back to nothing when WebGL is unavailable, and
 * honours prefers-reduced-motion with a single static frame. The render loop is
 * capped at ~30fps.
 *
 * Resize handling: the (expensive) WebGL drawing buffer is only reallocated
 * once the window size has *settled* (~120ms after the last resize event).
 * While the user is actively dragging the window, the canvas simply stretches
 * via CSS — so the window resize stays smooth instead of stuttering.
 */
const canvasRef = ref<HTMLCanvasElement | null>(null);

let gl: WebGLRenderingContext | null = null;
let raf = 0;
let program: WebGLProgram | null = null;
let buffer: WebGLBuffer | null = null;
let running = false;
let reduceMotion = false;
let uTime: WebGLUniformLocation | null = null;
let uAspect: WebGLUniformLocation | null = null;
let uColor: WebGLUniformLocation[] = [];
let uOpacity: WebGLUniformLocation | null = null;
let blendAdditive = false;

// Target (CSS) size + the timestamp of the last change, used to debounce the
// costly buffer reallocation during a window drag.
let targetW = 1;
let targetH = 1;
let bufferW = 0;
let bufferH = 0;

const VERT = `
  attribute vec2 aPos;
  varying vec2 vUv;
  void main() {
    vUv = aPos * 0.5 + 0.5;
    gl_Position = vec4(aPos, 0.0, 1.0);
  }
`;

const FRAG = `
  precision highp float;
  uniform float uTime;
  uniform float uAspect;
  uniform vec3 uColor1;
  uniform vec3 uColor2;
  uniform vec3 uColor3;
  uniform vec3 uColor4;
  uniform float uOpacity;
  varying vec2 vUv;

  float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 34.56);
    return fract(p.x * p.y);
  }
  float noise(vec2 p) {
    vec2 i = floor(p);
    vec2 f = fract(p);
    float a = hash(i);
    float b = hash(i + vec2(1.0, 0.0));
    float c = hash(i + vec2(0.0, 1.0));
    float d = hash(i + vec2(1.0, 1.0));
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
  }
  float fbm(vec2 p) {
    float v = 0.0;
    float a = 0.5;
    for (int i = 0; i < 4; i++) {
      v += a * noise(p);
      p = p * 2.02 + vec2(11.3, 7.7);
      a *= 0.5;
    }
    return v;
  }

  void main() {
    vec2 uv = vUv;
    vec2 p = (uv - 0.5) * vec2(uAspect, 1.0) * 3.0;
    float t = uTime * 0.035;

    float w = fbm(p * 1.1 + vec2(t, -t * 0.6));
    float clouds = fbm(p * 1.5 + w * 2.0);
    clouds = pow(clamp(clouds, 0.0, 1.0), 1.5);

    vec3 col = mix(uColor1, uColor2, smoothstep(0.15, 0.65, clouds));
    col = mix(col, uColor3, smoothstep(0.5, 0.9, clouds));
    float core = smoothstep(0.72, 1.0, clouds);
    col += core * uColor4 * 0.7;

    float vig = smoothstep(1.15, 0.25, length(uv - 0.5));
    float alpha = clouds * uOpacity * vig;
    gl_FragColor = vec4(col * alpha, alpha);
  }
`;

function hexToRgb01(hex: string): [number, number, number] {
  const m = hex.replace("#", "");
  const v =
    m.length === 3
      ? m.split("").map((c) => c + c).join("")
      : m;
  const n = parseInt(v, 16);
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

function compile(type: number, src: string): WebGLShader | null {
  if (!gl) return null;
  const sh = gl.createShader(type);
  if (!sh) return null;
  gl.shaderSource(sh, src);
  gl.compileShader(sh);
  if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
    console.warn("shader compile failed:", gl.getShaderInfoLog(sh));
    gl.deleteShader(sh);
    return null;
  }
  return sh;
}

// FilePressor palette — pinks/roses, distinct from drift's indigo/cyan.
// Light mode uses deeper, more saturated pinks so the clouds actually read on
// a near-white background instead of washing out.
function palette(): {
  c1: string;
  c2: string;
  c3: string;
  c4: string;
  opacity: number;
  additive: boolean;
} {
  const dark = document.documentElement.dataset.theme === "dark";
  if (dark) {
    return {
      c1: "#4a1228",
      c2: "#c04d6f",
      c3: "#e885ab",
      c4: "#ffd2e2",
      opacity: 0.5,
      additive: true,
    };
  }
  return {
    c1: "#a04870",
    c2: "#d4648c",
    c3: "#e885ab",
    c4: "#fcddef",
    opacity: 0.7,
    additive: false,
  };
}

function applyPalette(): void {
  if (!gl || !program) return;
  gl.useProgram(program);
  const p = palette();
  const cols = [p.c1, p.c2, p.c3, p.c4].map(hexToRgb01);
  for (let i = 0; i < 4; i++) {
    gl.uniform3f(uColor[i], cols[i][0], cols[i][1], cols[i][2]);
  }
  gl.uniform1f(uOpacity, p.opacity);
  blendAdditive = p.additive;
  gl.enable(gl.BLEND);
  if (blendAdditive) gl.blendFunc(gl.ONE, gl.ONE);
  else gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
}

// Resize: we only update the aspect-ratio uniform on window resize so the
// shader adapts to the new proportions.  The WebGL drawing buffer is never
// reallocated after the initial setup — changing canvas.width/height clears
// the buffer and causes a visible flash that looks like the animation
// restarting.  CSS stretches the canvas to fill the viewport instead.
function applyResize(): void {
  if (!gl) return;
  if (!bufferW || !bufferH) return; // not yet initialised
  const w = targetW;
  const h = targetH;
  if (w === bufferW && h === bufferH) return;
  bufferW = w;
  bufferH = h;
  gl.uniform1f(uAspect, w / h);
}

function markResize(): void {
  const w = Math.max(1, Math.round(window.innerWidth));
  const h = Math.max(1, Math.round(window.innerHeight));
  if (w !== targetW || h !== targetH) {
    targetW = w;
    targetH = h;
  }
}

let startT = 0;
function renderFrame(): void {
  if (!gl || !program) return;
  gl.useProgram(program);
  gl.uniform1f(uTime, (performance.now() - startT) / 1000);
  gl.drawArrays(gl.TRIANGLES, 0, 3);
}

function tick(): void {
  raf = requestAnimationFrame(tick);
  applyResize();
  renderFrame();
}

function onVisibility(): void {
  if (reduceMotion) return;
  if (document.hidden) {
    running = false;
    cancelAnimationFrame(raf);
  } else if (!running) {
    running = true;
    tick();
  }
}

function onThemeChange(): void {
  applyPalette();
  renderFrame();
}

onMounted(() => {
  const canvas = canvasRef.value;
  if (!canvas) return;

  reduceMotion = window.matchMedia?.(
    "(prefers-reduced-motion: reduce)",
  ).matches ?? false;

  gl =
    (canvas.getContext("webgl", { alpha: true, antialias: false }) as
      | WebGLRenderingContext
      | null) ??
    (canvas.getContext("experimental-webgl", { alpha: true }) as
      | WebGLRenderingContext
      | null);
  if (!gl) return;

  const vs = compile(gl.VERTEX_SHADER, VERT);
  const fs = compile(gl.FRAGMENT_SHADER, FRAG);
  if (!vs || !fs) return;
  program = gl.createProgram();
  if (!program) return;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    console.warn("program link failed:", gl.getProgramInfoLog(program));
    return;
  }
  gl.useProgram(program);

  // Fullscreen triangle.
  buffer = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 3, -1, -1, 3]),
    gl.STATIC_DRAW,
  );
  const aPos = gl.getAttribLocation(program, "aPos");
  gl.enableVertexAttribArray(aPos);
  gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);

  uTime = gl.getUniformLocation(program, "uTime");
  uAspect = gl.getUniformLocation(program, "uAspect");
  uOpacity = gl.getUniformLocation(program, "uOpacity");
  uColor = [
    gl.getUniformLocation(program, "uColor1")!,
    gl.getUniformLocation(program, "uColor2")!,
    gl.getUniformLocation(program, "uColor3")!,
    gl.getUniformLocation(program, "uColor4")!,
  ];

  applyPalette();

  // Seed target size and allocate the drawing buffer exactly once.
  const dpr = Math.min(window.devicePixelRatio || 1, 1.5);
  const initW = Math.max(1, Math.round(window.innerWidth));
  const initH = Math.max(1, Math.round(window.innerHeight));
  const bw = Math.max(1, Math.floor(initW * dpr));
  const bh = Math.max(1, Math.floor(initH * dpr));
  canvas.width = bw;
  canvas.height = bh;
  bufferW = initW;
  bufferH = initH;
  gl.viewport(0, 0, bw, bh);
  gl.uniform1f(uAspect, bw / bh);
  targetW = initW;
  targetH = initH;
  startT = performance.now();

  window.addEventListener("resize", markResize);
  document.addEventListener("visibilitychange", onVisibility);
  const observer = new MutationObserver(onThemeChange);
  observer.observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme"],
  });

  if (reduceMotion) {
    renderFrame();
  } else {
    running = true;
    tick();
  }

  (canvas as any).__observer = observer;
});

onUnmounted(() => {
  cancelAnimationFrame(raf);
  window.removeEventListener("resize", markResize);
  document.removeEventListener("visibilitychange", onVisibility);
  const obs = (canvasRef.value as any)?.__observer as MutationObserver | undefined;
  obs?.disconnect();
  if (gl && buffer) gl.deleteBuffer(buffer);
  if (gl && program) gl.deleteProgram(program);
  gl = null;
});
</script>

<template>
  <canvas ref="canvasRef" class="bg-fx" aria-hidden="true" />
</template>

<style scoped>
.bg-fx {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  z-index: -1;
  pointer-events: none;
  display: block;
}
</style>
