<script setup lang="ts">
export interface Stat {
  label: string;
  value: string;
  highlight?: boolean;
}

defineProps<{ title: string; subtitle: string; stats: Stat[] }>();
</script>

<template>
  <div class="result-card">
    <div class="check">
      <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
        <path d="m4.5 12.5 5 5 10-11" />
      </svg>
    </div>

    <div class="result-head">
      <h3 class="result-title">{{ title }}</h3>
      <p class="result-subtitle" :title="subtitle">{{ subtitle }}</p>
    </div>

    <div class="stats">
      <div v-for="stat in stats" :key="stat.label" class="stat" :class="{ highlight: stat.highlight }">
        <span class="stat-value">{{ stat.value }}</span>
        <span class="stat-label">{{ stat.label }}</span>
      </div>
    </div>

    <div class="result-actions">
      <slot />
    </div>
  </div>
</template>

<style scoped>
.result-card {
  padding: 20px 22px;
  border-radius: 16px;
  border: 1px solid var(--success-border, #d7efe2);
  background: var(--success-bg, #f2fbf6);
  animation: pop 0.28s cubic-bezier(0.2, 0.9, 0.3, 1.2);
}

@keyframes pop {
  from {
    opacity: 0;
    transform: translateY(6px) scale(0.98);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.check {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 46px;
  border-radius: 50%;
  color: #fff;
  background: linear-gradient(135deg, var(--brand, #c04d6f), #d98ba3);
  box-shadow: 0 6px 18px -6px var(--brand-glow, rgba(192, 77, 111, 0.5));
  margin-bottom: 14px;
}

.result-title {
  margin: 0 0 4px;
  font-size: 16px;
  font-weight: 700;
  color: var(--text, #211b20);
}

.result-subtitle {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-muted, #8a8090);
  word-break: break-all;
}

.stats {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
  margin-top: 16px;
}

.stat {
  flex: 1;
  min-width: 110px;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px 14px;
  border-radius: 12px;
  background: var(--surface, #fff);
  border: 1px solid var(--border, #ece8ee);
}

.stat-value {
  font-size: 15px;
  font-weight: 700;
  font-variant-numeric: tabular-nums;
  color: var(--text, #211b20);
}

.stat.highlight .stat-value {
  color: var(--brand, #c04d6f);
}

.stat-label {
  font-size: 11px;
  color: var(--text-muted, #8a8090);
}

.result-actions {
  display: flex;
  gap: 10px;
  margin-top: 18px;
  flex-wrap: wrap;
}
</style>
