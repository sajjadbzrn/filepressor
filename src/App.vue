<script setup lang="ts">
import { onMounted, ref, watchEffect } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import CompressPanel from "./components/CompressPanel.vue";
import ExtractPanel from "./components/ExtractPanel.vue";
import AboutModal from "./components/AboutModal.vue";
import Background3D from "./components/Background3D.vue";
import TitleBar from "./components/TitleBar.vue";
import { activeTab, requestShellOpen } from "./lib/shell";
import type { ShellMode } from "./lib/shell";

const theme = ref<"light" | "dark">("light");
const aboutOpen = ref(false);

function loadTheme(): "light" | "dark" {
  const stored = localStorage.getItem("fp-theme");
  if (stored === "light" || stored === "dark") return stored;
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

theme.value = loadTheme();

watchEffect(() => {
  document.documentElement.dataset.theme = theme.value;
  localStorage.setItem("fp-theme", theme.value);
});

function toggleTheme(): void {
  theme.value = theme.value === "light" ? "dark" : "light";
}

onMounted(async () => {
  // Subscribe BEFORE reading pending opens so we never miss requests that the
  // shell forwards from other instances (one invocation per selected file).
  await listen<{ mode: ShellMode; paths: string[] }>("open-paths", (e) => {
    requestShellOpen(e.payload.mode, e.payload.paths);
  });

  // Paths the app was launched with (e.g. right-click → Compress). Buffered in
  // Rust so every selected file is captured, even the earliest ones.
  try {
    const pending = await invoke<{ mode: ShellMode; paths: string[] }[]>(
      "take_pending_open",
    );
    for (const req of pending ?? []) {
      if (req?.paths?.length) requestShellOpen(req.mode, req.paths);
    }
  } catch {
    // Command unavailable — ignore.
  }

  // Pull the window to the front when launched from Explorer (otherwise it
  // opens behind File Explorer).
  try {
    await getCurrentWindow().setFocus();
  } catch {
    // ignore
  }
});
</script>

<template>
  <Background3D />
  <TitleBar
    :theme="theme"
    @toggle-theme="toggleTheme"
    @open-about="aboutOpen = true"
  />
  <div class="app">

    <nav class="tabs" role="tablist" aria-label="Mode">
      <button
        type="button"
        role="tab"
        :aria-selected="activeTab === 'compress'"
        :class="{ active: activeTab === 'compress' }"
        @click="activeTab = 'compress'"
      >
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M4 8V6a2 2 0 0 1 2-2h3l1.5 2H18a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-1" />
          <path d="M4 8h13a1.5 1.5 0 0 1 1.4 2l-2.6 6a1.5 1.5 0 0 1-1.4 1H4z" />
        </svg>
        Compress
      </button>
      <button
        type="button"
        role="tab"
        :aria-selected="activeTab === 'extract'"
        :class="{ active: activeTab === 'extract' }"
        @click="activeTab = 'extract'"
      >
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 3v13" />
          <path d="m6 11 6 6 6-6" />
          <path d="M4 21h16" />
        </svg>
        Extract
      </button>
    </nav>

    <main>
      <CompressPanel v-show="activeTab === 'compress'" />
      <ExtractPanel v-show="activeTab === 'extract'" />
    </main>

    <footer>
      Built on Rust · zstd · LZMA2 — archives open and re-compress without any external apps
    </footer>

    <AboutModal :open="aboutOpen" @close="aboutOpen = false" />
  </div>
</template>

<style scoped>
.app {
  max-width: 860px;
  margin: 0 auto;
  padding: 6px 28px 20px;
  display: flex;
  flex-direction: column;
  min-height: calc(100vh - 44px);
  animation: appIn 0.5s cubic-bezier(0.2, 0.8, 0.3, 1) both;
}

@keyframes appIn {
  from {
    opacity: 0;
    transform: translateY(8px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

.tabs {
  display: inline-flex;
  gap: 4px;
  padding: 4px;
  margin: 0 auto 22px;
  border-radius: 14px;
  background: var(--seg-bg, #f1edf2);
  border: 1px solid var(--border, #ece8ee);
}

.tabs button {
  display: inline-flex;
  align-items: center;
  gap: 7px;
  padding: 9px 22px;
  border: none;
  border-radius: 11px;
  background: transparent;
  color: var(--text-muted, #8a8090);
  font-family: inherit;
  font-size: 13.5px;
  font-weight: 650;
  cursor: pointer;
  transition: background 0.16s ease, color 0.16s ease, box-shadow 0.16s ease;
}

.tabs button.active {
  background: var(--surface, #fff);
  color: var(--brand, #c04d6f);
  box-shadow: 0 2px 8px -2px rgba(0, 0, 0, 0.15);
}

main {
  flex: 1;
}

footer {
  margin-top: 26px;
  text-align: center;
  font-size: 11.5px;
  color: var(--text-muted, #8a8090);
  opacity: 0.85;
}
</style>
