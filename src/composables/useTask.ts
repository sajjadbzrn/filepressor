import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { TaskDonePayload, TaskErrorPayload, TaskProgress } from "../types";

/**
 * User-friendly error message mapping.
 * Maps technical error messages to human-readable ones.
 */
function friendlyError(msg: string): string {
  const lower = msg.toLowerCase();

  // File system errors
  if (lower.includes("permission denied") || lower.includes("os error 5")) {
    return "Permission denied — try running as administrator or close any apps that might be using the files.";
  }
  if (lower.includes("os error 32") || lower.includes("sharing violation")) {
    return "File is in use — close Explorer windows or other apps that might have the file open, then try again.";
  }
  if (lower.includes("os error 2") || lower.includes("file not found") || lower.includes("path does not exist")) {
    return "File not found — the file may have been moved or deleted.";
  }
  if (lower.includes("os error 112") || lower.includes("disk full") || lower.includes("no space")) {
    return "Not enough disk space — free up some space and try again.";
  }

  // FFmpeg errors
  if (lower.includes("ffmpeg not found")) {
    return "ffmpeg not found — install ffmpeg and make sure it's on your PATH.";
  }
  if (lower.includes("ffmpeg exited with code")) {
    const code = msg.match(/code\s*(\d+)/)?.[1];
    return `Encoding failed (exit code ${code ?? "?"}) — the input file may be corrupted or unsupported.`;
  }

  // Archive errors
  if (lower.includes("unsupported or unrecognized archive")) {
    return "This archive format is not supported — try converting it to ZIP, 7Z, or RAR first.";
  }
  if (lower.includes("no compatible media files")) {
    return "No compatible media files found for the selected format — check that you have the right file types.";
  }
  if (lower.includes("no files selected")) {
    return "No files selected — add some files or folders to compress.";
  }
  if (lower.includes("another task is already running")) {
    return "Another task is already running — wait for it to finish or cancel it first.";
  }
  if (lower.includes("another process using") || lower.includes("being used by another process")) {
    return "File is in use by another process — close any apps that might be using these files.";
  }

  // Network / general
  if (lower.includes("network") || lower.includes("timeout")) {
    return "Network error — check your connection and try again.";
  }

  // Return original if no match
  if (msg.length > 200) {
    return msg.slice(0, 200) + "…";
  }
  return msg;
}

/**
 * Runs one background task (compress or extract). The backend only allows a
 * single task at a time, and events are gated on `running` so panels that did
 * not start the task ignore them.
 */
export function useTask() {
  const running = ref(false);
  const progress = ref<TaskProgress | null>(null);
  const error = ref<string | null>(null);
  const cancelled = ref(false);
  const paused = ref(false);

  const doneHandlers: ((payload: TaskDonePayload) => void)[] = [];
  let unlisteners: (() => void)[] = [];

  async function start(command: string, args: Record<string, unknown>): Promise<string | null> {
    if (running.value) return "Another task is already running.";
    error.value = null;
    cancelled.value = false;
    paused.value = false;
    progress.value = null;
    running.value = true;
    try {
      await invoke(command, args);
      return null;
    } catch (e) {
      running.value = false;
      const raw = typeof e === "string" ? e : String(e);
      return friendlyError(raw);
    }
  }

  async function cancel(): Promise<void> {
    try {
      await invoke("cancel_task");
    } catch {
      // Cancellation is best-effort
    }
  }

  async function pause(): Promise<void> {
    paused.value = true;
    try {
      await invoke("pause_task");
    } catch {
      paused.value = false;
    }
  }

  async function resume(): Promise<void> {
    paused.value = false;
    try {
      await invoke("resume_task");
    } catch {
      paused.value = true;
    }
  }

  /** Clears the error reported by the backend (used by the alert close button). */
  function clearError(): void {
    error.value = null;
  }

  function onDone(handler: (payload: TaskDonePayload) => void): void {
    doneHandlers.push(handler);
  }

  onMounted(async () => {
    unlisteners.push(
      await listen<TaskProgress>("task:progress", (event) => {
        if (running.value) progress.value = event.payload;
      }),
    );
    unlisteners.push(
      await listen<TaskDonePayload>("task:done", (event) => {
        if (!running.value) return;
        running.value = false;
        paused.value = false;
        doneHandlers.forEach((h) => h(event.payload));
      }),
    );
    unlisteners.push(
      await listen<TaskErrorPayload>("task:error", (event) => {
        if (!running.value) return;
        running.value = false;
        paused.value = false;
        cancelled.value = event.payload.cancelled;
        error.value = friendlyError(event.payload.message);
      }),
    );
  });

  onUnmounted(() => {
    unlisteners.forEach((u) => u());
    unlisteners = [];
  });

  return { running, progress, error, cancelled, paused, start, cancel, pause, resume, clearError, onDone };
}
