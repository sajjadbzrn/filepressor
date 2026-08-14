import { ref, shallowRef } from "vue";
import { check } from "@tauri-apps/plugin-updater";
import type { Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

const AUTO_KEY = "fp-auto-update";

// Shared singleton state so the in-app banner and the About modal stay in sync.
const checking = ref(false);
const update = shallowRef<Update | null>(null);
const updateAvailable = ref(false);
const downloading = ref(false);
const downloaded = ref(0);
const contentLength = ref(0);
const error = ref<string | null>(null);

// Auto-check on launch is on by default; the user can disable it in About.
const autoCheck = ref(localStorage.getItem(AUTO_KEY) !== "false");

let lastCheck = 0;

export function useUpdater() {
  function setAutoCheck(value: boolean): void {
    autoCheck.value = value;
    localStorage.setItem(AUTO_KEY, value ? "true" : "false");
  }

  async function checkForUpdates(): Promise<boolean> {
    // Debounce so rapid calls (e.g. modal open + launch) don't stack.
    const now = Date.now();
    if (checking.value || now - lastCheck < 2000) return updateAvailable.value;
    lastCheck = now;

    checking.value = true;
    error.value = null;
    try {
      const found = await check();
      if (found) {
        update.value = found;
        updateAvailable.value = true;
      } else {
        update.value = null;
        updateAvailable.value = false;
      }
      return updateAvailable.value;
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      return false;
    } finally {
      checking.value = false;
    }
  }

  async function installUpdate(): Promise<void> {
    if (!update.value || downloading.value) return;
    downloading.value = true;
    downloaded.value = 0;
    contentLength.value = 0;
    error.value = null;
    try {
      await update.value.downloadAndInstall((event) => {
        switch (event.event) {
          case "Started":
            contentLength.value = event.data.contentLength ?? 0;
            break;
          case "Progress":
            downloaded.value += event.data.chunkLength;
            break;
        }
      });
      // The installer on Windows exits the app; on other platforms relaunch.
      await relaunch();
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e);
      downloading.value = false;
    }
  }

  function dismiss(): void {
    updateAvailable.value = false;
    update.value = null;
  }

  return {
    checking,
    updateAvailable,
    update,
    downloading,
    downloaded,
    contentLength,
    error,
    autoCheck,
    setAutoCheck,
    checkForUpdates,
    installUpdate,
    dismiss,
  };
}
