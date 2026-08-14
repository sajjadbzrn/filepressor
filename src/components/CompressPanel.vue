<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { CompressResult, FileInfo, FormatSuggestion, TaskDonePayload } from "../types";
import { pendingOpen } from "../lib/shell";
import {
  dirname,
  ext,
  formatBytes,
  formatDuration,
  norm,
  stem,
} from "../lib/utils";
import { LEVELS, OUTPUT_FORMATS } from "../lib/constants";
import { pickFiles, pickFolders, pickSavePath } from "../lib/dialogs";
import { useFileDrop } from "../composables/useFileDrop";
import { useTask } from "../composables/useTask";
import DropZone from "./DropZone.vue";
import FileList from "./FileList.vue";
import ProgressBlock from "./ProgressBlock.vue";
import ResultCard from "./ResultCard.vue";
import SegmentedControl from "./SegmentedControl.vue";

const files = ref<FileInfo[]>([]);
const format = ref<string>("zip");
const level = ref<string>("balanced");
const outputPath = ref<string | null>(null);
const outputTouched = ref(false);
const result = ref<CompressResult | null>(null);
const errorMsg = ref<string | null>(null);
const suggestion = ref<FormatSuggestion | null>(null);
const suggestionApplied = ref(false);

const { running, progress, error, paused, start, cancel, pause, resume, clearError, onDone } =
  useTask();
const { hovering } = useFileDrop(handleDropped);

// Opened via the OS (Explorer right-click / CLI args).
watch(pendingOpen, async (p) => {
  if (p && p.mode === "compress" && !running.value && p.paths.length) {
    await addPaths(p.paths);
    pendingOpen.value = null;
  }
});

const selectedFormat = computed(
  () => OUTPUT_FORMATS.find((f) => f.id === format.value) ?? OUTPUT_FORMATS[0],
);
const levelNote = computed(() =>
  selectedFormat.value.media
    ? "H.265 uses CRF + speed preset · AVIF uses CRF + cpu-used for the quality/size trade-off"
    : "ZIP & TAR.GZ use DEFLATE · TAR.ZST uses zstd · 7Z uses LZMA2 for the smallest possible file",
);
const totalSize = computed(() => files.value.reduce((s, f) => s + f.size, 0));
const hasArchives = computed(() => files.value.some((f) => f.kind === "archive"));
const canRun = computed(
  () => files.value.length > 0 && !!effectiveOutput.value && !running.value,
);

const autoOutput = computed<string | null>(() => {
  const first = files.value[0];
  if (!first) return null;
  const dir = dirname(first.path);
  let name = stem(first.name);
  const outExt = selectedFormat.value.outExt;
  const candidate = `${dir}/${name}${outExt}`;
  if (
    ext(first.name) === outExt.slice(1) ||
    files.value.some((f) => norm(f.path) === candidate)
  ) {
    name += "_recompressed";
  }
  return `${dir}/${name}${outExt}`;
});

const effectiveOutput = computed<string | null>(
  () => (outputTouched.value && outputPath.value ? outputPath.value : autoOutput.value),
);

async function handleDropped(paths: string[]): Promise<void> {
  if (running.value) return;
  await addPaths(paths);
}

async function addPaths(paths: string[]): Promise<void> {
  const existing = new Set(files.value.map((f) => norm(f.path)));
  const fresh = paths.filter((p) => !existing.has(norm(p)));
  if (!fresh.length) return;
  try {
    const infos = await invoke<FileInfo[]>("analyze_paths", { paths: fresh });
    files.value.push(...infos);
    result.value = null;
    // Request a format suggestion based on the newly added files.
    try {
      const sug = await invoke<FormatSuggestion>("suggest_format", {
        paths: files.value.map((f) => f.path),
      });
      suggestion.value = sug;
      // Auto-apply only on first add or if user hasn't changed the format yet.
      if (!suggestionApplied.value) {
        format.value = sug.format;
        suggestionApplied.value = true;
      }
    } catch {
      // Non-fatal — suggestion is purely advisory.
    }
  } catch (e) {
    errorMsg.value = typeof e === "string" ? e : String(e);
  }
}

async function browse(): Promise<void> {
  const picked = await pickFiles("Choose files to compress");
  if (picked?.length) await addPaths(picked);
}

async function browseFolders(): Promise<void> {
  const picked = await pickFolders("Choose folders to compress");
  if (picked?.length) await addPaths(picked);
}

function removeAt(i: number): void {
  files.value.splice(i, 1);
  result.value = null;
}

async function changeOutput(): Promise<void> {
  const picked = await pickSavePath(effectiveOutput.value ?? "archive.zip", [
    {
      name: `${selectedFormat.value.label} output`,
      extensions: [selectedFormat.value.outExt.slice(1)],
    },
  ]);
  if (picked) {
    outputPath.value = picked;
    outputTouched.value = true;
  }
}

async function compress(): Promise<void> {
  const output = effectiveOutput.value;
  if (!output) return;
  errorMsg.value = null;
  result.value = null;
  const err = await start("start_compress", {
    paths: files.value.map((f) => f.path),
    output,
    format: format.value,
    level: level.value,
  });
  if (err) errorMsg.value = err;
}

onDone((payload: TaskDonePayload) => {
  if (payload.kind !== "compress") return;
  result.value = payload.result as CompressResult;
});

async function openFolder(): Promise<void> {
  const p = result.value?.output ?? effectiveOutput.value;
  if (p) await revealItemInDir(p);
}

function reset(): void {
  files.value = [];
  result.value = null;
  outputTouched.value = false;
  outputPath.value = null;
  errorMsg.value = null;
  suggestion.value = null;
  suggestionApplied.value = false;
}

function savedPercent(): string | null {
  const r = result.value;
  if (!r || !r.originalSize) return null;
  const saved = 1 - r.outputSize / r.originalSize;
  return `${(saved * 100).toFixed(1)}%`;
}
</script>

<template>
  <section class="panel">
    <!-- Empty state -->
    <template v-if="!files.length && !running">
      <DropZone
        :hovering="hovering"
        :disabled="running"
        title="Drop files & folders here"
        subtitle="Anything — documents, images, videos, whole folders. Archives are re-compressed automatically, no WinRAR or other apps needed."
        @browse="browse"
      />
      <div class="empty-browse">
        <button class="btn btn-ghost" type="button" @click="browse">Browse files…</button>
        <button class="btn btn-ghost" type="button" @click="browseFolders">Browse folder…</button>
      </div>
      <div class="hint-row">
        <span class="hint-chip">ZIP</span>
        <span class="hint-chip">7Z</span>
        <span class="hint-chip">RAR</span>
        <span class="hint-chip">TAR</span>
        <span class="hint-chip">GZ</span>
        <span class="hint-chip">BZ2</span>
        <span class="hint-chip">XZ</span>
        <span class="hint-chip">ZST</span>
        <span class="hint-chip">H.265</span>
        <span class="hint-chip">AVIF</span>

      </div>
    </template>

    <!-- Selection state -->
    <template v-else>
      <DropZone
        compact
        :hovering="hovering"
        :disabled="running"
        title="Drop more or click to add"
        subtitle="Archives are re-compressed automatically"
        @browse="browse"
      />

      <FileList :items="files" @remove="removeAt" />

      <div class="add-row">
        <button class="btn btn-ghost btn-sm" type="button" @click="browse">+ Add files…</button>
        <button class="btn btn-ghost btn-sm" type="button" @click="browseFolders">+ Add folder…</button>
      </div>

      <div class="meta-row">
        <span>{{ files.length }} item{{ files.length === 1 ? "" : "s" }} · {{ formatBytes(totalSize) }} total</span>
        <span v-if="hasArchives" class="recompress-note">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M9 9l-4 4 4 4" /><path d="M15 9l4 4-4 4" />
          </svg>
          Archive inputs are opened &amp; re-compressed
        </span>
      </div>

      <!-- Options -->
      <div class="options">
        <div class="option">
          <label class="option-label">Format</label>
          <SegmentedControl
            :options="OUTPUT_FORMATS.map((f) => ({ id: f.id, label: f.label, desc: f.desc }))"
            :model-value="format"
            @update:model-value="(v) => { format = v; result = null; }"
          />
          <p v-if="suggestion" class="option-suggestion">
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20z" />
              <path d="M12 16v-4" />
              <circle cx="12" cy="8" r="0.6" fill="currentColor" />
            </svg>
            {{ suggestion.reason }}
          </p>
          <p class="option-note">{{ selectedFormat.note }}</p>
        </div>

        <div class="option">
          <label class="option-label">Level</label>
          <SegmentedControl
            :options="LEVELS.map((l) => ({ id: l.id, label: l.label, desc: l.desc }))"
            :model-value="level"
            @update:model-value="level = $event"
          />
          <p class="option-note">{{ levelNote }}</p>
        </div>
      </div>

      <!-- Output -->
      <div class="output-row">
        <span class="output-label">Save as</span>
        <code class="output-path" :title="effectiveOutput ?? ''">{{ effectiveOutput ?? "—" }}</code>
        <button class="btn btn-ghost" type="button" @click="changeOutput">Change…</button>
      </div>

      <button class="btn btn-primary btn-big" type="button" :disabled="!canRun" @click="compress">
        Compress {{ files.length }} item{{ files.length === 1 ? "" : "s" }}
      </button>
    </template>

    <!-- Error -->
    <div v-if="(errorMsg || error) && !running" class="error-banner">
      <span>{{ errorMsg ?? error }}</span>
      <button type="button" class="error-close" @click="errorMsg = null; clearError()">✕</button>
    </div>

    <!-- Progress -->
    <ProgressBlock
      v-if="running"
      :progress="progress"
      :paused="paused"
      @cancel="cancel"
      @pause="pause"
      @resume="resume"
    />

    <!-- Result -->
    <ResultCard
      v-if="result"
      title="Compression complete"
      :subtitle="result.output"
      :stats="[
        { label: 'Original', value: formatBytes(result.originalSize) },
        { label: 'Compressed', value: formatBytes(result.outputSize), highlight: true },
        { label: 'Saved', value: savedPercent() ?? '—' },
        { label: 'Time', value: formatDuration(result.durationMs) },
      ]"
    >
      <button class="btn btn-primary" type="button" @click="openFolder">Open folder</button>
      <button class="btn btn-ghost" type="button" @click="reset">Compress more</button>
    </ResultCard>
  </section>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  animation: panelIn 0.4s cubic-bezier(0.2, 0.8, 0.3, 1) both;
}

@keyframes panelIn {
  from {
    opacity: 0;
    transform: translateY(6px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.empty-browse {
  display: flex;
  justify-content: center;
  gap: 10px;
  margin-top: 4px;
}

.empty-browse {
  display: flex;
  justify-content: center;
  gap: 10px;
  margin-top: 4px;
}

.hint-row {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  flex-wrap: wrap;
  margin-top: 2px;
}

.hint-chip {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.05em;
  padding: 3px 8px;
  border-radius: 999px;
  border: 1px solid var(--brand-soft, #f8e9ee);
  color: var(--text-muted, #8a8090);
  background: var(--brand-soft, #f8e9ee);
}

.meta-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  font-size: 12px;
  color: var(--text-muted, #8a8090);
  padding: 0 2px;
}

.recompress-note {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  color: var(--brand, #c04d6f);
  font-weight: 600;
}

.add-row {
  display: flex;
  gap: 8px;
}

.btn-sm {
  padding: 7px 14px;
  font-size: 12.5px;
  border-radius: 10px;
}

.options {
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 14px;
  border-radius: 14px;
  border: 1px solid var(--border, #ece8ee);
  background: var(--surface, #fff);
}

.option-label {
  display: block;
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted, #8a8090);
  margin-bottom: 7px;
}

.option-note {
  margin: 7px 2px 0;
  font-size: 11.5px;
  color: var(--text-muted, #8a8090);
}

.option-suggestion {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  margin: 7px 2px 0;
  padding: 5px 10px;
  border-radius: 8px;
  font-size: 11.5px;
  font-weight: 600;
  color: var(--brand, #c04d6f);
  background: var(--brand-soft, #f8e9ee);
}

.output-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px;
  border-radius: 12px;
  background: var(--surface, #fff);
  border: 1px solid var(--border, #ece8ee);
}

.output-label {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-muted, #8a8090);
  flex-shrink: 0;
}

.output-path {
  flex: 1;
  min-width: 0;
  font-size: 12.5px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  color: var(--text, #211b20);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.error-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 11px 14px;
  border-radius: 12px;
  border: 1px solid #f3c3cd;
  background: #fdf0f3;
  color: #b02a4a;
  font-size: 13px;
}

.error-close {
  border: none;
  background: transparent;
  color: inherit;
  cursor: pointer;
  font-size: 13px;
}
</style>
