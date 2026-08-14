<script setup lang="ts">
import { ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";

defineProps<{
  theme: "light" | "dark";
}>();

const emit = defineEmits<{
  "toggle-theme": [];
  "open-about": [];
}>();

const win = getCurrentWindow();
const isMaximized = ref(false);
const hoveredBtn = ref<string | null>(null);

// Track maximize state
win.onResized(async () => {
  isMaximized.value = await win.isMaximized();
});

async function minimize() {
  await win.minimize();
}

async function toggleMaximize() {
  if (isMaximized.value) {
    await win.unmaximize();
  } else {
    await win.maximize();
  }
}

async function close() {
  await win.close();
}
</script>

<template>
  <div class="titlebar">
    <!-- macOS-style traffic lights -->
    <div class="traffic-lights" data-tauri-drag-region="false">
      <button
        class="tl-btn tl-close"
        type="button"
        title="Close"
        data-tauri-drag-region="false"
        @mouseenter="hoveredBtn = 'close'"
        @mouseleave="hoveredBtn = null"
        @click="close"
      >
        <svg v-if="hoveredBtn === 'close'" viewBox="0 0 12 12" width="8" height="8">
          <path d="M3.5 3.5l5 5M8.5 3.5l-5 5" stroke="#4D0000" stroke-width="1.2" stroke-linecap="round" />
        </svg>
      </button>
      <button
        class="tl-btn tl-minimize"
        type="button"
        title="Minimize"
        data-tauri-drag-region="false"
        @mouseenter="hoveredBtn = 'minimize'"
        @mouseleave="hoveredBtn = null"
        @click="minimize"
      >
        <svg v-if="hoveredBtn === 'minimize'" viewBox="0 0 12 12" width="8" height="8">
          <path d="M2.5 6h7" stroke="#995700" stroke-width="1.2" stroke-linecap="round" />
        </svg>
      </button>
      <button
        class="tl-btn tl-maximize"
        type="button"
        :title="isMaximized ? 'Restore' : 'Maximize'"
        data-tauri-drag-region="false"
        @mouseenter="hoveredBtn = 'maximize'"
        @mouseleave="hoveredBtn = null"
        @click="toggleMaximize"
      >
        <svg v-if="hoveredBtn === 'maximize'" viewBox="0 0 12 12" width="8" height="8">
          <path v-if="!isMaximized" d="M3 3h6v6H3z" stroke="#006500" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" fill="none" />
          <path v-else d="M4.5 2.5v5h5M7.5 7.5h2.5v-2.5" stroke="#006500" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" fill="none" />
        </svg>
      </button>
    </div>

    <!-- Drag zone: title area -->
    <div class="titlebar-drag" data-tauri-drag-region>
      <span class="titlebar-label">FilePressor</span>
    </div>

    <!-- Right actions -->
    <div class="titlebar-actions" data-tauri-drag-region="false">
      <button
        class="tb-icon"
        type="button"
        title="About FilePressor"
        data-tauri-drag-region="false"
        @click="emit('open-about')"
      >
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
          <circle cx="12" cy="12" r="9" />
          <path d="M12 11v5" />
          <circle cx="12" cy="8" r="0.6" fill="currentColor" />
        </svg>
      </button>
      <button
        class="tb-icon"
        type="button"
        :title="theme === 'light' ? 'Switch to dark mode' : 'Switch to light mode'"
        data-tauri-drag-region="false"
        @click="emit('toggle-theme')"
      >
        <svg v-if="theme === 'light'" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round">
          <circle cx="12" cy="12" r="4.2" />
          <path d="M12 2.5v2.4M12 19.1v2.4M2.5 12h2.4M19.1 12h2.4M5.3 5.3l1.7 1.7M17 17l1.7 1.7M18.7 5.3 17 7M7 17l-1.7 1.7" />
        </svg>
        <svg v-else viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round">
          <path d="M20.5 14.5A8.5 8.5 0 0 1 9.5 3.5a8.5 8.5 0 1 0 11 11z" />
        </svg>
      </button>
    </div>
  </div>
</template>

<style scoped>
.titlebar {
  position: sticky;
  top: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  height: 44px;
  padding: 0 14px;
  user-select: none;
  background: transparent;
}

/* ---- Traffic lights ---- */
.traffic-lights {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 2px;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
  position: relative;
  z-index: 10;
}

.tl-btn {
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: none;
  padding: 0;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: filter 0.15s ease, transform 0.1s ease;
  -webkit-app-region: no-drag;
  position: relative;
  z-index: 10;
}

.tl-btn:active {
  transform: scale(0.85);
}

.tl-close {
  background: #ff5f57;
}
.tl-minimize {
  background: #febc2e;
}
.tl-maximize {
  background: #28c840;
}

.tl-close:hover {
  background: #ff5f57;
  filter: brightness(0.9);
}
.tl-minimize:hover {
  background: #febc2e;
  filter: brightness(0.9);
}
.tl-maximize:hover {
  background: #28c840;
  filter: brightness(0.9);
}

/* ---- Drag zone (title area) ---- */
.titlebar-drag {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  -webkit-app-region: drag;
  height: 100%;
}

.titlebar-label {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-muted, #8a8090);
  letter-spacing: -0.01em;
  opacity: 0.7;
  pointer-events: none;
}

/* ---- Right actions ---- */
.titlebar-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  -webkit-app-region: no-drag;
  position: relative;
  z-index: 10;
}

.tb-icon {
  width: 30px;
  height: 30px;
  border-radius: 8px;
  border: none;
  background: transparent;
  color: var(--text-muted, #8a8090);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  transition: background 0.15s ease, color 0.15s ease;
  -webkit-app-region: no-drag;
  position: relative;
  z-index: 10;
}

.tb-icon:hover {
  background: var(--border, #b8a8b6);
  color: var(--text, #211b20);
}
</style>
