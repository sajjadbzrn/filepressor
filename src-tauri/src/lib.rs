mod archives;
mod media;
mod models;
mod tasks;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use models::{ArchiveEntry, FileInfo, FormatSuggestion};
use tauri::{Emitter, Manager, State};
use walkdir::WalkDir;

/// A request coming from the OS (right-click context menu / CLI args):
/// open the app directly into compress/extract with the given paths.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct OpenRequest {
    mode: String,
    paths: Vec<String>,
}

/// Holds launch args (Explorer context menu / CLI) until the frontend asks for
/// them. Requests accumulate so that when the shell invokes the verb once per
/// selected file (MultiSelectModel = Document) every path is captured — even
/// the ones that arrive before the frontend has subscribed to `open-paths`.
#[derive(Default)]
struct PendingOpen(pub Mutex<Vec<OpenRequest>>);

/// Bring the main window to the foreground (used when launched from Explorer).
fn focus_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Parses `--compress <paths...>` / `--extract <paths...>` from process args.
///
/// Handles edge cases from Windows Explorer context-menu invocations:
/// - `%1` / `%*` tokens that weren't expanded by the shell.
/// - Quoted paths that arrive with surrounding double-quotes.
/// - Paths containing spaces.
fn parse_open_request(args: &[String]) -> Option<OpenRequest> {
    let mut mode: Option<String> = None;
    let mut paths: Vec<String> = Vec::new();
    for a in args {
        if a == "--compress" {
            mode = Some("compress".into());
        } else if a == "--extract" {
            mode = Some("extract".into());
        } else if a.starts_with("--") {
            mode = None;
        } else if mode.is_some() {
            // Strip surrounding quotes Windows sometimes adds.
            let cleaned = a.trim_matches('"').trim().to_string();
            // Skip unexpanded shell tokens (they indicate the command wasn't invoked
            // properly — the context menu should use "%1" not %*).
            if cleaned.is_empty() || cleaned == "%1" || cleaned == "%*" {
                continue;
            }
            paths.push(cleaned);
        }
    }
    mode.and_then(|m| {
        if paths.is_empty() {
            None
        } else {
            Some(OpenRequest { mode: m, paths })
        }
    })
}

/// Returns metadata about each selected path (file / folder / archive).
#[tauri::command]
async fn analyze_paths(paths: Vec<String>) -> Result<Vec<FileInfo>, String> {
    archives::analyze_paths(&paths)
}

/// Lists the entries inside an archive (for the preview).
#[tauri::command]
async fn list_archive(path: String) -> Result<Vec<ArchiveEntry>, String> {
    archives::list_archive(&path)
}

/// Analyzes the input paths and suggests the best compression format.
#[tauri::command]
async fn suggest_format(paths: Vec<String>) -> Result<FormatSuggestion, String> {
    let mut image_count = 0u64;
    let mut video_count = 0u64;
    let mut other_count = 0u64;
    let mut total_files = 0u64;
    let has_ffmpeg = media::ffmpeg_available();

    for p in &paths {
        let path = PathBuf::from(p);
        if !path.exists() {
            continue;
        }
        if path.is_dir() {
            for entry in WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                total_files += 1;
                let fp = entry.path();
                if media::is_image(fp) {
                    image_count += 1;
                } else if media::is_video(fp) {
                    video_count += 1;
                } else if archives::detect_archive(fp).is_some() {
                    other_count += 1;
                } else {
                    other_count += 1;
                }
            }
        } else if archives::detect_archive(&path).is_some() {
            other_count += 1;
            total_files += 1;
        } else if media::is_image(&path) {
            image_count += 1;
            total_files += 1;
        } else if media::is_video(&path) {
            video_count += 1;
            total_files += 1;
        } else {
            other_count += 1;
            total_files += 1;
        }
    }

    // All images → AVIF (requires ffmpeg)
    if total_files > 0 && image_count == total_files {
        if has_ffmpeg {
            return Ok(FormatSuggestion {
                format: "avif".into(),
                reason: format!("All {total_files} file{ps} {are} images — AVIF gives the smallest output with great quality.",
                    ps = if total_files == 1 { "" } else { "s" },
                    are = if total_files == 1 { "is" } else { "are" }),
            });
        } else {
            return Ok(FormatSuggestion {
                format: "7z".into(),
                reason: format!("All {total_files} file{ps} {are} images. Install ffmpeg for AVIF support; 7Z is the next best.",
                    ps = if total_files == 1 { "" } else { "s" },
                    are = if total_files == 1 { "is" } else { "are" }),
            });
        }
    }

    // All videos → H.265 (requires ffmpeg)
    if total_files > 0 && video_count == total_files {
        if has_ffmpeg {
            return Ok(FormatSuggestion {
                format: "h265".into(),
                reason: format!(
                    "All {total_files} file{ps} {are} videos — H.265 shrinks them dramatically.",
                    ps = if total_files == 1 { "" } else { "s" },
                    are = if total_files == 1 { "is" } else { "are" }
                ),
            });
        } else {
            return Ok(FormatSuggestion {
                format: "7z".into(),
                reason: format!("All {total_files} file{ps} {are} videos. Install ffmpeg for H.265 support; 7Z is the next best.",
                    ps = if total_files == 1 { "" } else { "s" },
                    are = if total_files == 1 { "is" } else { "are" }),
            });
        }
    }

    // Majority images + ffmpeg → AVIF
    if image_count > 0 && image_count > video_count && image_count > other_count && has_ffmpeg {
        return Ok(FormatSuggestion {
            format: "avif".into(),
            reason: format!("Most files ({image_count} of {total_files}) {are} images — AVIF is optimal for image data.",
                are = if image_count == 1 { "is" } else { "are" }),
        });
    }

    // Majority videos + ffmpeg → H.265
    if video_count > 0 && video_count > image_count && video_count > other_count && has_ffmpeg {
        return Ok(FormatSuggestion {
            format: "h265".into(),
            reason: format!("Most files ({video_count} of {total_files}) {are} videos — H.265 is optimal for video data.",
                are = if video_count == 1 { "is" } else { "are" }),
        });
    }

    // Everything else → 7z (best general compression)
    Ok(FormatSuggestion {
        format: "7z".into(),
        reason: "7Z with LZMA2 gives the best compression ratio for mixed or general files.".into(),
    })
}

/// Starts a background compression task. Progress is reported via events.
#[tauri::command]
fn start_compress(
    app: tauri::AppHandle,
    state: State<'_, tasks::AppState>,
    paths: Vec<String>,
    output: String,
    format: String,
    level: String,
) -> Result<(), String> {
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err("Another task is already running".to_string());
    }
    if paths.is_empty() {
        state.busy.store(false, Ordering::Release);
        return Err("No files selected".to_string());
    }
    state.cancel.store(false, Ordering::Relaxed);
    state.paused.store(false, Ordering::Relaxed);
    tasks::run_compress_task(
        app,
        state.busy.clone(),
        state.cancel.clone(),
        state.paused.clone(),
        paths,
        output,
        format,
        level,
    );
    Ok(())
}

/// Starts a background extraction task. Progress is reported via events.
#[tauri::command]
fn start_extract(
    app: tauri::AppHandle,
    state: State<'_, tasks::AppState>,
    archive: String,
    dest: String,
) -> Result<(), String> {
    if state.busy.swap(true, Ordering::AcqRel) {
        return Err("Another task is already running".to_string());
    }
    state.cancel.store(false, Ordering::Relaxed);
    state.paused.store(false, Ordering::Relaxed);
    tasks::run_extract_task(
        app,
        state.busy.clone(),
        state.cancel.clone(),
        state.paused.clone(),
        archive,
        dest,
    );
    Ok(())
}

/// Requests cancellation of the running task.
#[tauri::command]
fn cancel_task(state: State<'_, tasks::AppState>) {
    state.cancel.store(true, Ordering::Relaxed);
}

/// Pauses the running task (the worker thread parks until resumed).
#[tauri::command]
fn pause_task(state: State<'_, tasks::AppState>) {
    state.paused.store(true, Ordering::Relaxed);
}

/// Resumes a paused task.
#[tauri::command]
fn resume_task(state: State<'_, tasks::AppState>) {
    state.paused.store(false, Ordering::Relaxed);
}

/// Returns (and clears) every path the app was launched with via the OS.
///
/// Returns a list because the shell may invoke the context-menu verb once per
/// selected file; all of those are buffered here and handed to the frontend in
/// one batch so nothing is lost.
#[tauri::command]
fn take_pending_open(state: State<'_, PendingOpen>) -> Vec<OpenRequest> {
    std::mem::take(&mut *state.0.lock().unwrap())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Capture launch args (e.g. opened via the Explorer context menu).
    let pending = parse_open_request(&std::env::args().collect::<Vec<_>>());
    let mut pending_buf = Vec::new();
    if let Some(req) = pending {
        pending_buf.push(req);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // App already running — forward the request to the live window and
            // pull it to the front so the user sees it over File Explorer.
            if let Some(req) = parse_open_request(&argv) {
                if let Some(state) = app.try_state::<PendingOpen>() {
                    state.0.lock().unwrap().push(req.clone());
                }
                let _ = app.emit("open-paths", &req);
            }
            focus_main(app);
        }))
        .manage(tasks::AppState::default())
        .manage(PendingOpen(Mutex::new(pending_buf)))
        .setup(|_app| {
            // Set the title-bar icon on Windows from the embedded PNG.
            #[cfg(target_os = "windows")]
            {
                use tauri::Manager;
                if let Some(window) = _app.get_webview_window("main") {
                    let icon =
                        tauri::image::Image::from_bytes(include_bytes!("../icons/128x128.png"))?;
                    window.set_icon(icon)?;
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            analyze_paths,
            list_archive,
            suggest_format,
            start_compress,
            start_extract,
            cancel_task,
            pause_task,
            resume_task,
            take_pending_open
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
