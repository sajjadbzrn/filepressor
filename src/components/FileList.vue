<script setup lang="ts">
import type { FileInfo } from "../types";
import { formatBytes } from "../lib/utils";
import { ARCHIVE_BADGES } from "../lib/constants";

defineProps<{ items: FileInfo[] }>();
const emit = defineEmits<{ remove: [index: number] }>();

function icon(kind: FileInfo["kind"]): string {
  switch (kind) {
    case "archive":
      return "archive";
    case "folder":
      return "folder";
    default:
      return "file";
  }
}
</script>

<template>
  <ul class="file-list">
    <li v-for="(item, i) in items" :key="item.path" class="file-row">
      <span class="file-icon" :class="item.kind" aria-hidden="true">
        <!-- folder -->
        <svg v-if="icon(item.kind) === 'folder'" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        </svg>
        <!-- archive -->
        <svg v-else-if="icon(item.kind) === 'archive'" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <rect x="3" y="4" width="18" height="16" rx="2" />
          <path d="M3 10h18" />
          <path d="M8 4v6" />
          <path d="M12 4v6" />
          <path d="M16 4v6" />
        </svg>
        <!-- file -->
        <svg v-else viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
          <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
          <path d="M14 3v5h5" />
        </svg>
      </span>

      <span class="file-name" :title="item.path">{{ item.name }}</span>

      <span v-if="item.kind === 'archive' && item.archiveFormat" class="badge" :class="'badge-' + item.archiveFormat.replace('.', '-')">
        {{ ARCHIVE_BADGES[item.archiveFormat] ?? item.archiveFormat.toUpperCase() }}
      </span>
      <span v-else-if="item.kind === 'folder'" class="badge badge-folder">Folder</span>

      <span class="file-size">{{ formatBytes(item.size) }}</span>

      <button class="remove" type="button" title="Remove" @click="emit('remove', i)">
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M6 6l12 12M18 6L6 18" />
        </svg>
      </button>
    </li>
  </ul>
</template>

<style scoped>
.file-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: 240px;
  overflow-y: auto;
}

.file-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: 12px;
  background: var(--surface, #fff);
  border: 1px solid var(--border, #ece8ee);
  transition: border-color 0.15s ease;
}

.file-row:hover {
  border-color: var(--brand, #c04d6f);
}

.file-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 9px;
  flex-shrink: 0;
}

.file-icon.file {
  color: #7a6f82;
  background: var(--chip-bg, #f2eff4);
}

.file-icon.folder {
  color: #b8860b;
  background: #faf3e2;
}

.file-icon.archive {
  color: var(--brand, #c04d6f);
  background: var(--brand-soft, #f8e9ee);
}

.file-name {
  font-size: 13.5px;
  font-weight: 550;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
  flex: 1;
}

.badge {
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.04em;
  padding: 3px 8px;
  border-radius: 999px;
  color: var(--brand, #c04d6f);
  background: var(--brand-soft, #f8e9ee);
  flex-shrink: 0;
}

.badge-folder {
  color: #a5760a;
  background: #faf3e2;
}

.file-size {
  font-size: 12px;
  color: var(--text-muted, #8a8090);
  font-variant-numeric: tabular-nums;
  flex-shrink: 0;
  min-width: 68px;
  text-align: right;
}

.remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text-muted, #8a8090);
  cursor: pointer;
  flex-shrink: 0;
  transition: background 0.15s ease, color 0.15s ease;
}

.remove:hover {
  background: #fbe9ee;
  color: var(--brand-strong, #a93a5e);
}
</style>
