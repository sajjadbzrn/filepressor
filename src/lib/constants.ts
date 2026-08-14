export interface OutputFormat {
  id: "zip" | "7z" | "tgz" | "tzst" | "h265" | "avif";
  label: string;
  desc: string;
  note: string;
  /** File extension written for a single-file output, e.g. ".zip". */
  outExt: string;
  /** True for ffmpeg media transcoding (H.265 / AVIF) instead of archiving. */
  media: boolean;
}

export const OUTPUT_FORMATS: OutputFormat[] = [
  { id: "zip", label: "ZIP", desc: "Universal", note: "Works everywhere — email, cloud, Windows Explorer", outExt: ".zip", media: false },
  { id: "7z", label: "7Z", desc: "Smallest", note: "LZMA2 — the best compression ratio", outExt: ".7z", media: false },
  { id: "tgz", label: "TAR.GZ", desc: "Great ratio", note: "Classic gzip — tiny + fast", outExt: ".tar.gz", media: false },
  { id: "tzst", label: "TAR.ZST", desc: "Fastest", note: "zstd — near-instant, still very small", outExt: ".tar.zst", media: false },
  { id: "h265", label: "H.265", desc: "Video", note: "Re-encode video to HEVC — tiny files, great quality. Uses ffmpeg (libx265).", outExt: ".mp4", media: true },
  { id: "avif", label: "AVIF", desc: "Image", note: "Convert images to AVIF — smaller than PNG/JPEG at equal quality. Uses ffmpeg (libaom).", outExt: ".avif", media: true },
];

export interface Level {
  id: "fast" | "balanced" | "maximum";
  label: string;
  desc: string;
}

export const LEVELS: Level[] = [
  { id: "fast", label: "Fast", desc: "Quick, still small" },
  { id: "balanced", label: "Balanced", desc: "Best speed / size" },
  { id: "maximum", label: "Maximum", desc: "Smallest output" },
];

export const ARCHIVE_BADGES: Record<string, string> = {
  zip: "ZIP",
  "7z": "7Z",
  rar: "RAR",
  tar: "TAR",
  "tar.gz": "TGZ",
  "tar.bz2": "TBZ2",
  "tar.xz": "TXZ",
  "tar.zst": "TZST",
  gz: "GZ",
  bz2: "BZ2",
  xz: "XZ",
  zst: "ZST",
};
