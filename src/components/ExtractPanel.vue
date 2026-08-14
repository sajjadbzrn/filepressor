<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import type { ArchiveEntry, ExtractResult, FileInfo, TaskDonePayload } from "../types";
import { pendingOpen } from "../lib/shell";
import {
  basename,
  dirname,
  formatBytes,
  formatDuration,
  norm,
  stem,
} from "../lib/utils";
import { ARCHIVE_BADGES } from "../lib/constants";
import { pickFiles, pickFolder } from "../lib/dialogs";
import { useFileDrop } from "../composables/useFileDrop";
import { useTask } from "../composables/useTask";
import DropZone from "./DropZone.vue";
import ProgressBlock from "./ProgressBlock.vue";
import ResultCard from "./ResultCard.vue";

const archive = ref<FileInfo | null>(null);
const entries = ref<ArchiveEntry[] | null>(null);
const entryError = ref<string | null>(null);
const destPath = ref<string | null>(null);
const destTouched = ref(false);
const result = ref<ExtractResult | null>(null);
const errorMsg = ref<string | null>(null);
const loading = ref(false);

const { running, progress, error, paused, start, cancel, pause, resume, clearError, onDone } =
  useTask();
const { hovering } = useFileDrop(handleDropped);

// Opened via the OS (Explorer right-click / CLI args).
watch(pendingOpen, async (p) => {
  if (p && p.mode === "extract" && !running.value && p.paths.length) {
    await setArchive(p.paths[0]);
    pendingOpen.value = null;
  }
});

const badge = computed(() =>
  archive.value?.archiveFormat
    ? (ARCHIVE_BADGES[archive.value.archiveFormat] ?? archive.value.archiveFormat.toUpperCase())
    : null,
);

const totalEntriesSize = computed(() =>
  entries.value?.reduce((s, e) => s + (e.isDir ? 0 : e.size), 0) ?? 0,
);

const fileCount = computed(
  () => entries.value?.filter((e) => !e.isDir).length ?? 0,
);

const preview = computed(() => entries.value?.slice(0, 60) ?? []);
const previewMore = computed(
  () => (entries.value?.length ?? 0) - preview.value.length,
);

const autoDest = computed<string | null>(() => {
  if (!archive.value) return null;
  return `${dirname(archive.value.path)}/${stem(archive.value.name)}`;
});

const effectiveDest = computed<string | null>(
  () => (destTouched.value && destPath.value ? destPath.value : autoDest.value),
);

async function handleDropped(paths: string[]): Promise<void> {
  if (running.value) return;
  const p = paths[0];
  if (p) await setArchive(p);
}

async function browse(): Promise<void> {
  const picked = await pickFiles("Choose an archive to extract");
  if (picked?.length) await setArchive(picked[0]);
}

async function setArchive(path: string): Promise<void> {
  loading.value = true;
  entryError.value = null;
  result.value = null;
  errorMsg.value = null;
  destTouched.value = false;
  destPath.value = null;
  try {
    const infos = await invoke<FileInfo[]>("analyze_paths", { paths: [norm(path)] });
    const info = infos[0];
    if (!info) return;
    if (info.isDir || info.kind !== "archive") {
      errorMsg.value = "That's not an archive — drop a ZIP, 7Z, RAR, TAR, GZ, BZ2, XZ or ZST file.";
      archive.value = null;
      entries.value = null;
      return;
    }
    archive.value = info;
    const list = await invoke<ArchiveEntry[]>("list_archive", { path: info.path });
    entries.value = list;
  } catch (e) {
    archive.value = null;
    entries.value = null;
    entryError.value = typeof e === "string" ? e : String(e);
  } finally {
    loading.value = false;
  }
}

async function changeDest(): Promise<void> {
  const picked = await pickFolder("Choose a destination folder");
  if (picked) {
    destPath.value = picked;
    destTouched.value = true;
  }
}

async function extract(): Promise<void> {
  if (!archive.value || !effectiveDest.value) return;
  errorMsg.value = null;
  result.value = null;
  const err = await start("start_extract", {
    archive: archive.value.path,
    dest: effectiveDest.value,
  });
  if (err) errorMsg.value = err;
}

onDone((payload: TaskDonePayload) => {
  if (payload.kind !== "extract") return;
  result.value = payload.result as ExtractResult;
});

async function openFolder(): Promise<void> {
  const p = result.value?.dest ?? effectiveDest.value;
  if (p) await revealItemInDir(p);
}

function reset(): void {
  archive.value = null;
  entries.value = null;
  result.value = null;
  destTouched.value = false;
  destPath.value = null;
  errorMsg.value = null;
  entryError.value = null;
}
</script>

<template>
  <section class="panel">
    <!-- Empty state -->
    <template v-if="!archive && !running">
      <DropZone
        :hovering="hovering"
        :disabled="running"
        title="Drop an archive here"
        subtitle="ZIP, 7Z, RAR, TAR, TAR.GZ, TAR.BZ2, TAR.XZ, TAR.ZST, GZ, BZ2, XZ, ZST — FilePressor opens them all, no external apps."
        @browse="browse"
      />
    </template>

    <!-- Archive selected -->
    <template v-else-if="archive">
      <DropZone
        compact
        :hovering="hovering"
        :disabled="running"
        title="Drop another archive to switch"
        subtitle=""
        @browse="browse"
      />

      <div class="archive-card">
        <span class="archive-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="4" width="18" height="16" rx="2" />
            <path d="M3 10h18" />
            <path d="M8 4v6" /><path d="M12 4v6" /><path d="M16 4v6" />
          </svg>
        </span>
        <div class="archive-info">
          <span class="archive-name" :title="archive.path">{{ basename(archive.path) }}</span>
          <span class="archive-meta">{{ formatBytes(archive.size) }} · {{ archive.path }}</span>
        </div>
        <span v-if="badge" class="badge">{{ badge }}</span>
      </div>

      <!-- Contents preview -->
      <div class="contents">
        <div class="contents-head">
          <span class="contents-title">Contents</span>
          <span v-if="entries" class="contents-meta">
            {{ fileCount }} file{{ fileCount === 1 ? "" : "s" }} · {{ formatBytes(totalEntriesSize) }}
          </span>
          <span v-else-if="entryError" class="contents-meta contents-error">{{ entryError }}</span>
        </div>

        <ul v-if="preview.length" class="contents-list">
          <li v-for="e in preview" :key="e.path" class="contents-row">
            <svg v-if="e.isDir" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
            </svg>
            <svg v-else viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" /><path d="M14 3v5h5" />
            </svg>
            <span class="entry-name" :title="e.path">{{ e.path }}</span>
            <span class="entry-size">{{ e.isDir ? "" : formatBytes(e.size) }}</span>
          </li>
          <li v-if="previewMore > 0" class="contents-more">…and {{ previewMore }} more entries</li>
        </ul>
        <p v-else-if="!entries && !entryError" class="contents-loading">
          {{ loading ? "Reading archive…" : "" }}
        </p>
      </div>

      <!-- Destination -->
      <div class="output-row">
        <span class="output-label">Extract to</span>
        <code class="output-path" :title="effectiveDest ?? ''">{{ effectiveDest ?? "—" }}</code>
        <button class="btn btn-ghost" type="button" @click="changeDest">Change…</button>
      </div>

      <button class="btn btn-primary btn-big" type="button" :disabled="!archive || !effectiveDest || running" @click="extract">
        Extract {{ fileCount || "" }} file{{ fileCount === 1 ? "" : "s" }}
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
      title="Extraction complete"
      :subtitle="result.dest"
      :stats="[
        { label: 'Files extracted', value: String(result.entries) },
        { label: 'Time', value: formatDuration(result.durationMs) },
      ]"
    >
      <button class="btn btn-primary" type="button" @click="openFolder">Open folder</button>
      <button class="btn btn-ghost" type="button" @click="reset">Extract another</button>
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

.archive-card {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid var(--border, #ece8ee);
  background: var(--surface, #fff);
}

.archive-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 40px;
  height: 40px;
  border-radius: 11px;
  color: var(--brand, #c04d6f);
  background: var(--brand-soft, #f8e9ee);
  flex-shrink: 0;
}

.archive-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.archive-name {
  font-size: 14px;
  font-weight: 650;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.archive-meta {
  font-size: 11.5px;
  color: var(--text-muted, #8a8090);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.badge {
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.05em;
  padding: 4px 10px;
  border-radius: 999px;
  color: var(--brand, #c04d6f);
  background: var(--brand-soft, #f8e9ee);
  flex-shrink: 0;
}

.contents {
  padding: 12px 14px;
  border-radius: 14px;
  border: 1px solid var(--border, #ece8ee);
  background: var(--surface, #fff);
}

.contents-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 8px;
}

.contents-title {
  font-size: 11.5px;
  font-weight: 700;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted, #8a8090);
}

.contents-meta {
  font-size: 12px;
  color: var(--text-muted, #8a8090);
}

.contents-error {
  color: #b02a4a;
}

.contents-list {
  list-style: none;
  margin: 0;
  padding: 0;
  max-height: 180px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.contents-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px;
  border-radius: 8px;
  font-size: 12.5px;
  color: var(--text-muted, #8a8090);
}

.contents-row:hover {
  background: var(--seg-bg, #f1edf2);
}

.entry-name {
  flex: 1;
  min-width: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.entry-size {
  font-variant-numeric: tabular-nums;
  font-size: 11.5px;
  flex-shrink: 0;
}

.contents-more {
  padding: 4px 6px;
  font-size: 12px;
  color: var(--brand, #c04d6f);
  font-weight: 600;
}

.contents-loading {
  margin: 8px 0 4px;
  font-size: 12.5px;
  color: var(--text-muted, #8a8090);
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
