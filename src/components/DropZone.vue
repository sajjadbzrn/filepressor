<script setup lang="ts">
defineProps<{
  compact?: boolean;
  hovering?: boolean;
  disabled?: boolean;
  title: string;
  subtitle?: string;
}>();

const emit = defineEmits<{ browse: [] }>();
</script>

<template>
  <button
    class="dropzone"
    :class="{ compact, hovering, disabled }"
    type="button"
    :disabled="disabled"
    @click="emit('browse')"
  >
    <span class="dz-icon" aria-hidden="true">
      <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
        <path d="M12 16V4" />
        <path d="m7 9 5-5 5 5" />
        <path d="M4 15v3a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-3" />
      </svg>
    </span>
    <span class="dz-title">{{ title }}</span>
    <span v-if="subtitle" class="dz-subtitle">{{ subtitle }}</span>
    <span class="dz-action">Browse files…</span>
  </button>
</template>

<style scoped>
.dropzone {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  width: 100%;
  padding: 44px 24px;
  border: 1.5px dashed var(--brand, #c04d6f);
  border-radius: 18px;
  background: var(--dropzone-bg, #fdf7f9);
  color: var(--text, #211b20);
  cursor: pointer;
  transition: background 0.18s ease, border-color 0.18s ease, transform 0.12s ease, box-shadow 0.18s ease;
  font-family: inherit;
}

.dropzone.compact {
  flex-direction: row;
  justify-content: flex-start;
  gap: 14px;
  padding: 14px 20px;
  border-radius: 14px;
}

.dropzone:hover:not(:disabled),
.dropzone.hovering {
  background: var(--dropzone-bg-hover, #fbebf1);
  border-color: var(--brand-strong, #a93a5e);
  box-shadow: 0 6px 24px -10px var(--brand-glow, rgba(192, 77, 111, 0.45));
}

.dropzone.hovering {
  transform: scale(1.005);
}

.dropzone:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.dz-icon {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 48px;
  height: 48px;
  border-radius: 14px;
  color: var(--brand, #c04d6f);
  background: var(--brand-soft, #f8e9ee);
  flex-shrink: 0;
}

.dz-title {
  font-size: 15px;
  font-weight: 650;
}

.dz-subtitle {
  font-size: 12.5px;
  color: var(--text-muted, #8a8090);
  max-width: 46ch;
  text-align: center;
}

.dropzone.compact .dz-title {
  font-size: 14px;
}

.dropzone.compact .dz-subtitle {
  text-align: left;
}

.dz-action {
  margin-top: 8px;
  font-size: 12.5px;
  font-weight: 600;
  color: var(--brand, #c04d6f);
  padding: 5px 12px;
  border-radius: 999px;
  background: var(--brand-soft, #f8e9ee);
}

.dropzone.compact .dz-action {
  margin-top: 0;
  margin-left: auto;
}
</style>
