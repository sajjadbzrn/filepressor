import { onMounted, onUnmounted, ref } from "vue";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * Native drag & drop of OS files (works on Windows/Linux/macOS via Tauri).
 * The drop event is window-wide, so panels just call `onDrop` with paths.
 */
export function useFileDrop(onDrop: (paths: string[]) => void) {
  const hovering = ref(false);
  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    try {
      unlisten = await getCurrentWebview().onDragDropEvent((event) => {
        const p = event.payload;
        if (p.type === "enter" || p.type === "over") {
          hovering.value = true;
        } else if (p.type === "leave") {
          hovering.value = false;
        } else if (p.type === "drop") {
          hovering.value = false;
          onDrop(p.paths);
        }
      });
    } catch {
      hovering.value = false;
    }
  });

  onUnmounted(() => {
    unlisten?.();
  });

  return { hovering };
}
