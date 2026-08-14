/** Normalize Windows backslashes so the frontend can treat paths uniformly. */
export function norm(p: string): string {
  return p.replace(/\\/g, "/");
}

export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.min(units.length - 1, Math.floor(Math.log(n) / Math.log(1024)));
  const v = n / 1024 ** i;
  const digits = i === 0 || v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return `${v.toFixed(digits)} ${units[i]}`;
}

export function formatDuration(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "–";
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(1)} s`;
  return `${Math.floor(s / 60)}m ${Math.round(s % 60)}s`;
}

export function dirname(p: string): string {
  const n = norm(p);
  const i = n.lastIndexOf("/");
  return i <= 0 ? n : n.slice(0, i);
}

/** File name without the directory. */
export function basename(p: string): string {
  const n = norm(p);
  const i = n.lastIndexOf("/");
  return i === -1 ? n : n.slice(i + 1);
}

/** Everything before the last dot (keeps multi-part extensions like .tar.gz intact). */
export function stem(p: string): string {
  const n = basename(p);
  const i = n.lastIndexOf(".");
  return i <= 0 ? n : n.slice(0, i);
}

export function ext(p: string): string {
  const n = basename(p);
  const i = n.lastIndexOf(".");
  return i <= 0 ? "" : n.slice(i + 1).toLowerCase();
}

/** The archive extension family of a format id, e.g. "7z" -> ".7z". */
export function formatExtension(formatId: string): string {
  switch (formatId) {
    case "zip":
      return ".zip";
    case "7z":
      return ".7z";
    case "tgz":
      return ".tar.gz";
    case "tzst":
      return ".tar.zst";
    default:
      return ".zip";
  }
}
