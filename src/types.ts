export interface FileInfo {
  path: string;
  name: string;
  size: number;
  isDir: boolean;
  kind: "archive" | "file" | "folder";
  archiveFormat: string | null;
}

export interface ArchiveEntry {
  path: string;
  size: number;
  isDir: boolean;
}

export interface TaskProgress {
  phase: "scan" | "extract" | "compress";
  current: number;
  total: number;
  file: string;
}

export interface CompressResult {
  output: string;
  outputSize: number;
  originalSize: number;
  entries: number;
  durationMs: number;
}

export interface ExtractResult {
  dest: string;
  entries: number;
  durationMs: number;
}

export interface TaskDonePayload {
  kind: "compress" | "extract";
  result: CompressResult | ExtractResult;
}

export interface TaskErrorPayload {
  message: string;
  cancelled: boolean;
}

export interface FormatSuggestion {
  format: string;
  reason: string;
}
