//! Media transcoding (H.265 video / AVIF image) via an external `ffmpeg`.
//!
//! ffmpeg is located from the bundled resources dir first, then `PATH`. When
//! the chosen format is `h265`/`avif` the backend extracts any archive inputs
//! to a temp dir first, then transcodes every matching media file. A single
//! media file is written to the chosen output path; multiple inputs (folders,
//! archives, several files) are written into an output directory that mirrors
//! the source tree, with non-media files copied through unchanged.

use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

/// On Windows, child processes (ffmpeg / ffprobe) spawn their own console
/// window unless we explicitly suppress it. That window is the "black CLI box"
/// that flashes open and closed every time we probe or transcode media.
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
fn hide_console_window(cmd: &mut Command) {
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_cmd: &mut Command) {}

/// Memoized ffmpeg discovery so we only shell out to probe it once per run.
static FFMPEG_CACHE: OnceLock<Option<PathBuf>> = OnceLock::new();

use walkdir::WalkDir;

use crate::archives::{extract_archive, detect_archive, Progress, CANCELLED};
use crate::models::CompressResult;

fn ext_of(p: &Path) -> String {
    p.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

pub fn is_video(p: &Path) -> bool {
    matches!(
        ext_of(p).as_str(),
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "flv" | "wmv" | "m4v" | "mpg" | "mpeg" | "ts"
            | "3gp"
    )
}

pub fn is_image(p: &Path) -> bool {
    matches!(
        ext_of(p).as_str(),
        "png" | "jpg" | "jpeg" | "bmp" | "tif" | "tiff" | "webp" | "gif"
    )
}

/// Whether a file is a transcode target for the given media format.
pub fn is_media(p: &Path, format: &str) -> bool {
    match format {
        "h265" => is_video(p),
        "avif" => is_image(p),
        _ => false,
    }
}

/// Public check: is ffmpeg available on this system?
pub fn ffmpeg_available() -> bool {
    ffmpeg_path().is_some()
}

/// Find ffmpeg: a bundled copy next to the executable, else `PATH`.
/// The result is memoized so the `-version` probe runs at most once per launch.
fn ffmpeg_path() -> Option<PathBuf> {
    FFMPEG_CACHE
        .get_or_init(|| {
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    for name in ["ffmpeg", "ffmpeg.exe"] {
                        let p = dir.join(name);
                        if p.exists() {
                            return Some(p);
                        }
                    }
                }
            }
            let name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
            let mut cmd = Command::new(name);
            cmd.arg("-version").stdout(std::process::Stdio::null());
            hide_console_window(&mut cmd);
            if cmd.status().map(|s| s.success()).unwrap_or(false) {
                Some(PathBuf::from(name))
            } else {
                None
            }
        })
        .clone()
}

/// Get the duration of a media file in milliseconds via ffprobe.
/// Returns 0 if ffprobe is unavailable or the duration cannot be determined.
fn probe_duration(ffmpeg: &Path, input: &Path) -> u64 {
    let ffprobe = if let Some(dir) = ffmpeg.parent() {
        let name = if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        };
        let p = dir.join(name);
        if p.exists() {
            p
        } else {
            let name2 = if cfg!(windows) {
                "ffprobe.exe"
            } else {
                "ffprobe"
            };
            PathBuf::from(name2)
        }
    } else {
        let name = if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        };
        PathBuf::from(name)
    };

    let mut cmd = Command::new(&ffprobe);
    cmd.args([
        "-v", "error",
        "-show_entries", "format=duration",
        "-of", "default=noprint_wrappers=1:nokey=1",
    ])
    .arg(input)
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null());
    hide_console_window(&mut cmd);
    let output = cmd.output();

    match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            s.parse::<f64>()
                .map(|d| (d * 1000.0) as u64)
                .unwrap_or(0)
        }
        _ => 0,
    }
}

fn target_ext(format: &str) -> &'static str {
    match format {
        "h265" => ".mp4",
        _ => ".avif",
    }
}

/// Make sure `p` ends with `ext` (e.g. ".mp4"), replacing any existing one.
fn ensure_ext(p: &Path, ext: &str) -> PathBuf {
    let want = ext.trim_start_matches('.');
    if ext_of(p).as_str() == want {
        return p.to_path_buf();
    }
    p.with_extension(want)
}

fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Collect every file under `root`, relative to `base`, tagged with whether it
/// is a transcode target for `format`.
fn collect_from_dir(
    root: &Path,
    base: &Path,
    out: &mut Vec<(PathBuf, String, bool)>,
    format: &str,
) {
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let rel = p.strip_prefix(base).unwrap_or(p);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        out.push((p.to_path_buf(), rel_str, is_media(p, format)));
    }
}

fn build_args(input: &Path, output: &Path, format: &str, level: &str) -> Vec<String> {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mut args = vec![
        "-y".into(),
        "-hide_banner".into(),
        "-i".into(),
        input.to_string_lossy().to_string(),
    ];
    match format {
        "h265" => {
            args.push("-c:v".into());
            args.push("libx265".into());
            args.push("-tag:v".into());
            args.push("hvc1".into());
            args.push("-threads".into());
            args.push(threads.to_string());
            args.push("-x265-params".into());
            args.push(format!("pools={threads}:frame-threads={threads}"));
            let crf = match level {
                "fast" => 30,
                "maximum" => 24,
                _ => 28,
            };
            args.push("-crf".into());
            args.push(crf.to_string());
            // Faster presets: ultrafast / fast / medium
            let preset = match level {
                "fast" => "ultrafast",
                "maximum" => "medium",
                _ => "fast",
            };
            args.push("-preset".into());
            args.push(preset.into());
            args.push("-c:a".into());
            args.push("aac".into());
            args.push("-b:a".into());
            args.push("128k".into());
            args.push("-movflags".into());
            args.push("+faststart".into());
        }
        "avif" => {
            args.push("-c:v".into());
            args.push("libaom-av1".into());
            args.push("-pix_fmt".into());
            args.push("yuv420p".into());
            let crf = match level {
                "fast" => 40,
                "maximum" => 24,
                _ => 32,
            };
            args.push("-crf".into());
            args.push(crf.to_string());
            let cpu_used = match level {
                "fast" => 8,
                "maximum" => 0,
                _ => 4,
            };
            args.push("-cpu-used".into());
            args.push(cpu_used.to_string());
        }
        _ => {}
    }
    // Force the muxer: the real output is written via a `.part` temp name whose
    // extension ffmpeg would not recognize, so we set the container explicitly.
    let muxer = match format {
        "h265" => "mp4",
        "avif" => "avif",
        _ => "",
    };
    if !muxer.is_empty() {
        args.push("-f".into());
        args.push(muxer.into());
    }
    args.push(output.to_string_lossy().to_string());
    args
}

/// Parse a `time=HH:MM:SS.ss` value from an ffmpeg progress line into milliseconds.
///
/// ffmpeg emits lines like:
/// ```text
/// frame=  123 fps= 45.2 ... time=00:01:23.45 ... bitrate=1234kbits/s
/// ```
fn parse_ffmpeg_time_ms(line: &str) -> Option<u64> {
    let idx = line.find("time=")?;
    let rest = &line[idx + 5..];
    // time=HH:MM:SS.ss  or  time=SS.ss (older ffmpeg)
    let token: &str = rest.split_whitespace().next()?;
    let parts: Vec<&str> = token.split(':').collect();
    if parts.len() == 3 {
        let h: u64 = parts[0].parse().ok()?;
        let m: u64 = parts[1].parse().ok()?;
        let s: f64 = parts[2].parse().ok()?;
        Some(h * 3_600_000 + m * 60_000 + (s * 1000.0) as u64)
    } else if parts.len() == 1 {
        // Fallback: bare seconds (e.g. "time=12.34")
        let s: f64 = token.parse().ok()?;
        Some((s * 1000.0) as u64)
    } else {
        None
    }
}

/// Run ffmpeg for one file, polling cancel so the task can be aborted.
///
/// `file_duration_ms` is the duration of this specific input file (from ffprobe).
/// `cumulative_done_ms` is the total duration of all previously completed files.
/// `total_duration_ms` is the sum of all input file durations.
///
/// The parser thread reads `time=` from ffmpeg stderr and writes the current
/// position into a shared `AtomicU64`.  The main loop reads it every ~200 ms
/// to emit smooth, accurate progress to the frontend.
fn run_transcode(
    ffmpeg: &Path,
    input: &Path,
    output: &Path,
    format: &str,
    level: &str,
    cancel: &Arc<AtomicBool>,
    file_duration_ms: u64,
    cumulative_done_ms: u64,
    total_duration_ms: u64,
    progress: &mut Progress,
) -> Result<(), String> {
    // Copy the input to a temp file so Windows Explorer's thumbnail generator
    // cannot hold a lock on the original while ffmpeg reads it.
    let tmp_input = loop {
        let f = tempfile::NamedTempFile::new()
            .map_err(|e| format!("Failed to create temp file for staging: {e}"))?;
        match fs::copy(input, f.path()) {
            Ok(_) => break f,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied
                || e.raw_os_error() == Some(32) =>
            {
                std::thread::sleep(std::time::Duration::from_millis(300));
                continue;
            }
            Err(e) => return Err(format!("Failed to stage input for transcoding: {e}")),
        }
    };
    let staged_input = tmp_input.path();

    let part = output.with_extension(format!(
        "{}.part",
        ext_of(output).trim_start_matches('.')
    ));
    let args = build_args(staged_input, &part, format, level);
    let mut cmd = Command::new(ffmpeg);
    cmd.args(&args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    hide_console_window(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to launch ffmpeg: {e}"))?;

    // Shared atomic: the parser thread writes the latest `time=` value here,
    // and the main progress loop reads it every ~200 ms.
    let current_time_ms = Arc::new(AtomicU64::new(0));
    let current_time_ref = current_time_ms.clone();

    // Parse stderr in a background thread for time-based progress.
    let stderr = child.stderr.take();
    let cancel_clone = cancel.clone();
    let parse_handle = std::thread::spawn(move || {
        if let Some(reader) = stderr {
            let buf_reader = io::BufReader::new(reader);
            for line in buf_reader.lines() {
                if cancel_clone.load(Ordering::Relaxed) {
                    break;
                }
                if let Ok(line) = line {
                    if let Some(ms) = parse_ffmpeg_time_ms(&line) {
                        current_time_ref.store(ms, Ordering::Relaxed);
                    }
                }
            }
        }
    });

    let file_name = input
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut cancelled = false;
    let mut last_emit = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        match child.try_wait().map_err(|e| format!("{e}"))? {
            Some(status) => {
                let _ = parse_handle.join();
                // Emit a final 100% for this file.
                if total_duration_ms > 0 {
                    let done = cumulative_done_ms + file_duration_ms;
                    progress.emit("compress", done, total_duration_ms, &file_name);
                }
                if !status.success() {
                    let _ = fs::remove_file(&part);
                    return Err(format!("ffmpeg exited with code {status}"));
                }
                break;
            }
            None => {
                // Emit progress at most every 200 ms to avoid flooding.
                if last_emit.elapsed().as_millis() >= 200 {
                    let elapsed_ms = current_time_ms.load(Ordering::Relaxed);
                    // Clamp to file duration so we never exceed the current file's range.
                    let clamped = elapsed_ms.min(file_duration_ms);
                    if total_duration_ms > 0 {
                        let done = cumulative_done_ms + clamped;
                        progress.emit("compress", done, total_duration_ms, &file_name);
                    } else {
                        progress.emit("compress", elapsed_ms, file_duration_ms, &file_name);
                    }
                    last_emit = Instant::now();
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    if cancelled {
        let _ = fs::remove_file(&part);
        return Err(CANCELLED.to_string());
    }

    // Move the finished temp file onto the real output path.
    // On Windows, Explorer's thumbnail generator may briefly lock the destination
    // (especially for image formats like .avif), so retry a few times.
    if output.exists() {
        let _ = fs::remove_file(output);
    }
    for attempt in 0..10 {
        match fs::rename(&part, output) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let _ = e;
                std::thread::sleep(std::time::Duration::from_millis(200 * (attempt + 1)));
            }
        }
    }
    // Final fallback: copy then remove the temp.
    fs::copy(&part, output)
        .and_then(|_| fs::remove_file(&part).or_else(|_| Ok(())))
        .map_err(|e| format!("Could not finalize output {output:?}: {e}"))?;
    Ok(())
}

fn io_err(e: std::io::Error) -> String {
    format!("{e}")
}

pub fn compress_media(
    paths: &[String],
    output: &str,
    format: &str,
    level: &str,
    progress: &mut Progress,
) -> Result<CompressResult, String> {
    let started = Instant::now();
    let ffmpeg = ffmpeg_path().ok_or_else(|| {
        "ffmpeg not found. Install ffmpeg (with libx265 & libaom) and make sure it is on PATH."
            .to_string()
    })?;

    // Clone cancel Arc to avoid borrow conflicts with &mut Progress.
    let cancel = progress.cancel.clone();
    let _paused = progress.paused.clone();

    let tmp = tempfile::tempdir().map_err(io_err)?;
    let mut items: Vec<(PathBuf, String, bool)> = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        progress.check()?;
        let path = PathBuf::from(p);
        if !path.exists() {
            return Err(format!("Path does not exist: {p}"));
        }
        if path.is_dir() {
            collect_from_dir(&path, &path, &mut items, format);
        } else if detect_archive(&path).is_some() {
            let stem_name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("archive_{i}"));
            let stage = tmp.path().join(format!("{i}_{stem_name}"));
            fs::create_dir_all(&stage).map_err(io_err)?;
            progress.emit("extract", 0, 0, &format!("Opening {stem_name}…"));
            extract_archive(&path, &stage, progress)?;
            collect_from_dir(&stage, &stage, &mut items, format);
        } else if is_media(&path, format) {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("file_{i}"));
            items.push((path, name, true));
        }
    }

    let media_count = items.iter().filter(|(_, _, m)| *m).count();
    if media_count == 0 {
        return Err("No compatible media files found for the selected format.".into());
    }

    let out = PathBuf::from(output);
    let ext = target_ext(format);

    // Probe durations for all media files upfront to enable time-based progress.
    let mut total_duration_ms: u64 = 0;
    let mut file_durations: Vec<u64> = Vec::with_capacity(items.len());
    for (path, _rel, is_media_file) in &items {
        if *is_media_file {
            let dur = probe_duration(&ffmpeg, path);
            file_durations.push(dur);
            total_duration_ms += dur;
        } else {
            file_durations.push(0);
        }
    }

    // Single media file -> one output file at the chosen path.
    if items.len() == 1 && items[0].2 {
        let (src, _rel, _) = &items[0];
        let out_path = ensure_ext(&out, ext);
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        let dur = file_durations[0];
        progress.emit("compress", 0, dur, &src.file_name().unwrap().to_string_lossy());
        run_transcode(&ffmpeg, src, &out_path, format, level, &cancel, dur, 0, dur, progress)?;
        let original_size = fs::metadata(src).map_err(io_err)?.len();
        let output_size = fs::metadata(&out_path).map_err(io_err)?.len();
        return Ok(CompressResult {
            output: out_path.to_string_lossy().to_string(),
            output_size,
            original_size,
            entries: 1,
            duration_ms: started.elapsed().as_millis() as u64,
        });
    }

    // Multiple inputs (folder / archive / several files) -> output directory.
    let out_dir = if out.extension().is_some() {
        out.with_extension("")
    } else {
        out.clone()
    };
    fs::create_dir_all(&out_dir).map_err(io_err)?;

    let mut cumulative_done_ms: u64 = 0;
    let mut entries = 0u64;
    let mut original_size = 0u64;
    let mut file_idx = 0usize;
    for (src, rel, is_media_file) in &items {
        progress.check()?;
        let base = Path::new(rel);
        let sub = base.parent().unwrap_or_else(|| Path::new(""));
        let dest = if *is_media_file {
            let name = base
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "media".into());
            out_dir.join(sub).join(format!("{name}{ext}"))
        } else {
            out_dir.join(rel)
        };
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(io_err)?;
        }
        if *is_media_file {
            let dur = file_durations[file_idx];
            let file_name = src.file_name().unwrap_or_default().to_string_lossy();
            progress.emit("compress", cumulative_done_ms, total_duration_ms, &file_name);
            run_transcode(
                &ffmpeg, src, &dest, format, level, &cancel,
                dur, cumulative_done_ms, total_duration_ms, progress,
            )?;
            original_size += fs::metadata(src).map_err(io_err)?.len();
            cumulative_done_ms += dur;
            entries += 1;
        } else {
            // Retry copy for non-media files in case Explorer has a lock.
            for attempt in 0..10 {
                match fs::copy(src, &dest) {
                    Ok(_) => break,
                    Err(e) if e.raw_os_error() == Some(32) => {
                        if attempt == 9 {
                            return Err(io_err(e));
                        }
                        std::thread::sleep(std::time::Duration::from_millis(
                            200 * (attempt + 1),
                        ));
                    }
                    Err(e) => return Err(io_err(e)),
                }
            }
        }
        file_idx += 1;
    }

    let output_size = dir_size(&out_dir);
    Ok(CompressResult {
        output: out_dir.to_string_lossy().to_string(),
        output_size,
        original_size,
        entries,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}
