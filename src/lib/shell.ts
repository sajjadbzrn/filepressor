import { ref } from "vue";

export type ShellMode = "compress" | "extract";

/** Which tab is active — shared so OS-launched requests can switch tabs. */
export const activeTab = ref<ShellMode>("compress");

/**
 * A request delivered from the OS (Explorer context menu / CLI args).
 * Panels watch this and load the paths into the right mode.
 */
export const pendingOpen = ref<{ mode: ShellMode; paths: string[] } | null>(null);

/**
 * Handle a shell-open request (from context menu / CLI args).
 *
 * When the Windows Explorer integration uses `MultiSelectModel = Document`,
 * the shell invokes the verb once per selected file. Rapid invocations from
 * the single-instance plugin each call this function. We merge the paths so
 * the frontend receives them all — `addPaths` in CompressPanel deduplicates.
 */
export function requestShellOpen(mode: ShellMode, paths: string[]): void {
  activeTab.value = mode;
  const existing = pendingOpen.value;
  if (existing && existing.mode === mode) {
    // Merge with any pending paths from a rapid preceding invocation.
    const merged = [...new Set([...existing.paths, ...paths])];
    pendingOpen.value = { mode, paths: merged };
  } else {
    // Either the first request, or a switch of mode — start fresh with these.
    pendingOpen.value = { mode, paths: [...paths] };
  }
}
