//! Runs compression / extraction tasks on background threads and reports
//! progress, completion and errors to the frontend via Tauri events.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tauri::{AppHandle, Emitter};

use crate::archives::{self, Progress, CANCELLED};
use crate::models::{ExtractResult, TaskDone, TaskError, TaskProgress};

pub struct AppState {
    pub busy: Arc<AtomicBool>,
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            busy: Arc::new(AtomicBool::new(false)),
            cancel: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn make_progress(app: AppHandle, cancel: Arc<AtomicBool>, paused: Arc<AtomicBool>) -> Progress {
    Progress {
        cancel,
        paused,
        emit: Box::new(move |phase, current, total, file| {
            let _ = app.emit(
                "task:progress",
                TaskProgress {
                    phase: phase.to_string(),
                    current,
                    total,
                    file: file.to_string(),
                },
            );
        }),
    }
}

fn emit_finish(
    app: &AppHandle,
    busy: &Arc<AtomicBool>,
    result: Result<serde_json::Value, String>,
    kind: &str,
) {
    busy.store(false, Ordering::Release);
    match result {
        Ok(value) => {
            let _ = app.emit(
                "task:done",
                TaskDone {
                    kind: kind.to_string(),
                    result: value,
                },
            );
        }
        Err(msg) => {
            let cancelled = msg == CANCELLED;
            let _ = app.emit(
                "task:error",
                TaskError {
                    message: if cancelled {
                        "Operation cancelled".to_string()
                    } else {
                        msg
                    },
                    cancelled,
                },
            );
        }
    }
}

pub fn run_compress_task(
    app: AppHandle,
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    paths: Vec<String>,
    output: String,
    format: String,
    level: String,
) {
    std::thread::spawn(move || {
        let mut progress = make_progress(app.clone(), cancel.clone(), paused);
        let result = archives::compress_files(&paths, &output, &format, &level, &mut progress);
        let is_cancel = matches!(result, Err(ref e) if e == CANCELLED);
        let mapped = result.map(|res| {
            serde_json::to_value(&res).unwrap_or_else(|_| serde_json::json!({}))
        });
        // A cancelled compression leaves a half-written archive behind — remove it.
        if is_cancel {
            let _ = std::fs::remove_file(&output);
        }
        emit_finish(&app, &busy, mapped, "compress");
    });
}

pub fn run_extract_task(
    app: AppHandle,
    busy: Arc<AtomicBool>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    archive: String,
    dest: String,
) {
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut progress = make_progress(app.clone(), cancel.clone(), paused);
        let result = archives::extract_archive(
            std::path::Path::new(&archive),
            std::path::Path::new(&dest),
            &mut progress,
        );
        let is_cancel = matches!(result, Err(ref e) if e == CANCELLED);
        let mapped = result.map(|entries| {
            let res = ExtractResult {
                dest: dest.clone(),
                entries,
                duration_ms: started.elapsed().as_millis() as u64,
            };
            serde_json::to_value(&res).unwrap_or_else(|_| serde_json::json!({}))
        });
        // A cancelled extraction leaves a half-unpacked folder behind — remove it.
        if is_cancel {
            let _ = std::fs::remove_dir_all(&dest);
        }
        emit_finish(&app, &busy, mapped, "extract");
    });
}
