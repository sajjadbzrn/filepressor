use serde::Serialize;

/// Describes one input path (file / folder / archive) shown in the UI list.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    /// "archive" | "file" | "folder"
    pub kind: String,
    /// Lowercase format label for archives, e.g. "zip", "7z", "rar", "tar.gz"
    pub archive_format: Option<String>,
}

/// One entry inside an archive (for the preview listing).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
}

/// Progress event emitted while a task runs.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgress {
    /// "scan" | "extract" | "compress"
    pub phase: String,
    pub current: u64,
    pub total: u64,
    pub file: String,
}

/// Payload of the `task:done` event for a compression task.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompressResult {
    pub output: String,
    pub output_size: u64,
    pub original_size: u64,
    pub entries: u64,
    pub duration_ms: u64,
}

/// Payload of the `task:done` event for an extraction task.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExtractResult {
    pub dest: String,
    pub entries: u64,
    pub duration_ms: u64,
}

/// Payload of the `task:error` event.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskError {
    pub message: String,
    pub cancelled: bool,
}

/// Payload of the `task:done` event: which kind of task finished with what result.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TaskDone {
    pub kind: String,
    pub result: serde_json::Value,
}

/// Suggested best format for the given input files.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FormatSuggestion {
    /// The recommended format id: "zip", "7z", "tgz", "tzst", "h265", "avif".
    pub format: String,
    /// Human-readable reason for the suggestion.
    pub reason: String,
}
