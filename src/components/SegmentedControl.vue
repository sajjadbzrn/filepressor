<script setup lang="ts">
import { computed } from "vue";

interface Option {
  id: string;
  label: string;
  desc?: string;
}

const props = defineProps<{ options: Option[]; modelValue: string }>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();

const count = computed(() => props.options.length);
</script>

<template>
  <div class="segmented" role="tablist">
    <button
      v-for="opt in options"
      :key="opt.id"
      type="button"
      role="tab"
      :aria-selected="modelValue === opt.id"
      :class="{ active: modelValue === opt.id }"
      :style="{ flexBasis: `calc((100% - ${(count - 1) * 6}px) / ${count})` }"
      @click="emit('update:modelValue', opt.id)"
    >
      <span class="seg-label">{{ opt.label }}</span>
      <span v-if="opt.desc" class="seg-desc">{{ opt.desc }}</span>
    </button>
  </div>
</template>

<style scoped>
.segmented {
  display: flex;
  gap: 6px;
  padding: 4px;
  border-radius: 14px;
  background: var(--seg-bg, #f1edf2);
  border: 1px solid var(--border, #ece8ee);
}

button {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 1px;
  min-width: 0;
  padding: 9px 10px;
  border: none;
  border-radius: 11px;
  background: transparent;
  color: var(--text-muted, #8a8090);
  cursor: pointer;
  font-family: inherit;
  transition: background 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
}

button:hover {
  color: var(--text, #211b20);
}

button.active {
  background: var(--surface, #fff);
  color: var(--brand, #c04d6f);
  box-shadow: 0 2px 8px -2px rgba(0, 0, 0, 0.14);
}

.seg-label {
  font-size: 13.5px;
  font-weight: 650;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}

.seg-desc {
  font-size: 10.5px;
  opacity: 0.75;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 100%;
}
</style>
