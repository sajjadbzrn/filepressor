<script setup lang="ts">
import { computed } from "vue";
import type { TaskProgress } from "../types";

const props = defineProps<{ progress: TaskProgress | null; paused?: boolean }>();
const emit = defineEmits<{ cancel: []; pause: []; resume: [] }>();

const percent = computed<number | null>(() => {
  const p = props.progress;
  if (!p || !p.total || p.total <= 0) return null;
  return Math.min(100, Math.round((p.current / p.total) * 100));
});

const phaseLabel = computed(() => {
  if (props.paused) return "Paused";
  const p = props.progress;
  if (!p) return "Preparing…";
  switch (p.phase) {
    case "extract":
      return "Extracting archive…";
    case "compress":
      return "Compressing…";
    default:
      return "Scanning…";
  }
});

const style = computed(() => ({
  width: `${percent.value ?? 100}%`,
}));
</script>

<template>
  <div class="progress-block">
    <div class="progress-head">
      <span class="phase">
        <span class="spinner" aria-hidden="true"></span>
        {{ phaseLabel }}
      </span>
      <span v-if="percent !== null" class="percent">{{ percent }}%</span>
    </div>

    <div class="track" :class="{ indeterminate: percent === null }">
      <div class="fill" :style="style"></div>
    </div>

    <div class="progress-foot">
      <span class="file" :title="progress?.file ?? ''">
        {{ progress?.file || "Working…" }}
      </span>
      <div class="actions">
        <button
          v-if="!paused"
          class="pause"
          type="button"
          title="Pause"
          @click="emit('pause')"
        >
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <rect x="6" y="5" width="4" height="14" rx="1" />
            <rect x="14" y="5" width="4" height="14" rx="1" />
          </svg>
          Pause
        </button>
        <button v-else class="pause" type="button" title="Resume" @click="emit('resume')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
            <path d="M7 5l12 7-12 7z" />
          </svg>
          Resume
        </button>
        <button class="cancel" type="button" @click="emit('cancel')">Cancel</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.progress-block {
  padding: 16px 18px;
  border-radius: 14px;
  border: 1px solid var(--border, #ece8ee);
  background: var(--surface, #fff);
}

.progress-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 10px;
}

.phase {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 13px;
  font-weight: 600;
  color: var(--text, #211b20);
}

.spinner {
  width: 14px;
  height: 14px;
  border-radius: 50%;
  border: 2px solid var(--brand-soft, #f8e9ee);
  border-top-color: var(--brand, #c04d6f);
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.percent {
  font-size: 13px;
  font-weight: 700;
  color: var(--brand, #c04d6f);
  font-variant-numeric: tabular-nums;
}

.track {
  height: 8px;
  border-radius: 999px;
  background: var(--seg-bg, #f1edf2);
  overflow: hidden;
}

.track.indeterminate .fill {
  width: 40% !important;
  animation: slide 1.2s ease-in-out infinite;
}

@keyframes slide {
  0% {
    transform: translateX(-110%);
  }
  100% {
    transform: translateX(280%);
  }
}

.fill {
  height: 100%;
  border-radius: 999px;
  background: linear-gradient(90deg, var(--brand, #c04d6f), #d98ba3);
  transition: width 0.25s ease;
}

  .progress-foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: 10px;
  }

  .file {
    font-size: 12px;
    color: var(--text-muted, #8a8090);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .cancel {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted, #8a8090);
    background: var(--seg-bg, #f1edf2);
    border: none;
    border-radius: 8px;
    padding: 5px 12px;
    cursor: pointer;
    transition: color 0.15s ease, background 0.15s ease;
  }

  .cancel:hover {
    color: #b02a4a;
    background: #fbe4ea;
  }

  .pause {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    font-weight: 600;
    color: var(--brand, #c04d6f);
    background: var(--brand-soft, #f8e9ee);
    border: none;
    border-radius: 8px;
    padding: 5px 12px;
    cursor: pointer;
    transition: filter 0.15s ease, background 0.15s ease;
  }

  .pause:hover {
    filter: brightness(0.97);
  }
</style>
