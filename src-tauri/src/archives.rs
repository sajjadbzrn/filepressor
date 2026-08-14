//! Archive detection, listing, extraction and compression.
//!
//! Supported reading: zip, 7z, rar, tar, tar.gz, tar.bz2, tar.xz, tar.zst,
//! and single-file gzip / bzip2 / xz / zstd.
//! Supported writing: zip, 7z (LZMA2), tar.gz, tar.zst.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use walkdir::WalkDir;

use crate::models::{ArchiveEntry, CompressResult, FileInfo};

/// Sentinel error message used internally to signal a cancelled operation.
pub const CANCELLED: &str = "__cancelled__";

// ---------------------------------------------------------------------------
// Progress plumbing
// ---------------------------------------------------------------------------

/// Carries cancellation + pause state and emits progress events from worker threads.
pub struct Progress {
    pub cancel: Arc<AtomicBool>,
    pub paused: Arc<AtomicBool>,
    pub emit: Box<dyn FnMut(&str, u64, u64, &str) + Send>,
}

impl Progress {
    #[inline]
    pub fn check(&self) -> Result<(), String> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(CANCELLED.to_string());
        }
        self.park_if_paused();
        Ok(())
    }

    /// Blocks the worker thread while paused (cancel still wins and aborts).
    #[inline]
    pub fn park_if_paused(&self) {
        while self.paused.load(Ordering::Relaxed) {
            if self.cancel.load(Ordering::Relaxed) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
    }

    #[inline]
    pub fn emit(&mut self, phase: &str, current: u64, total: u64, file: &str) {
        (self.emit)(phase, current, total, file);
    }
}

/// Wraps a reader and reports byte-level progress, throttled to ~100 ms.
struct CountingReader<'a, R: Read> {
    inner: R,
    progress: &'a mut Progress,
    current: &'a mut u64,
    total: u64,
    phase: &'static str,
    file: String,
    last_emit: std::time::Instant,
    last_bytes: u64,
}

impl<'a, R: Read> CountingReader<'a, R> {
    fn new(
        inner: R,
        progress: &'a mut Progress,
        current: &'a mut u64,
        total: u64,
        phase: &'static str,
        file: String,
    ) -> Self {
        Self {
            inner,
            progress,
            current,
            total,
            phase,
            file,
            last_emit: std::time::Instant::now(),
            last_bytes: 0,
        }
    }
}

impl<'a, R: Read> Read for CountingReader<'a, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.progress.cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, CANCELLED));
        }
        // Pause mid-stream without losing the streaming writer's state.
        while self.progress.paused.load(Ordering::Relaxed) {
            if self.progress.cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, CANCELLED));
            }
            std::thread::sleep(std::time::Duration::from_millis(80));
        }
        let n = self.inner.read(buf)?;
        if n > 0 {
            *self.current += n as u64;
            let elapsed = self.last_emit.elapsed();
            // Emit at most every 100 ms or when the operation is complete.
            if elapsed.as_millis() >= 100
                || *self.current >= self.total
                || *self.current - self.last_bytes >= 2_097_152
            {
                self.progress
                    .emit(self.phase, *self.current, self.total, &self.file);
                self.last_emit = std::time::Instant::now();
                self.last_bytes = *self.current;
            }
        }
        Ok(n)
    }
}

#[inline]
fn io_err(e: io::Error) -> String {
    if e.kind() == io::ErrorKind::Interrupted && e.to_string() == CANCELLED {
        CANCELLED.to_string()
    } else {
        format!("{e}")
    }
}

#[inline]
fn zip_err(e: zip::result::ZipError) -> String {
    format!("{e}")
}

// ---------------------------------------------------------------------------
// Format detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKind {
    Zip,
    SevenZ,
    Rar,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    Gz,
    Bz2,
    Xz,
    Zst,
}

impl ArchiveKind {
    pub fn label(self) -> &'static str {
        match self {
            ArchiveKind::Zip => "zip",
            ArchiveKind::SevenZ => "7z",
            ArchiveKind::Rar => "rar",
            ArchiveKind::Tar => "tar",
            ArchiveKind::TarGz => "tar.gz",
            ArchiveKind::TarBz2 => "tar.bz2",
            ArchiveKind::TarXz => "tar.xz",
            ArchiveKind::TarZst => "tar.zst",
            ArchiveKind::Gz => "gz",
            ArchiveKind::Bz2 => "bz2",
            ArchiveKind::Xz => "xz",
            ArchiveKind::Zst => "zst",
        }
    }
}

fn detect_by_ext(path: &Path) -> Option<ArchiveKind> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return Some(ArchiveKind::TarGz);
    }
    if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") || name.ends_with(".tbz") {
        return Some(ArchiveKind::TarBz2);
    }
    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        return Some(ArchiveKind::TarXz);
    }
    if name.ends_with(".tar.zst") || name.ends_with(".tzst") {
        return Some(ArchiveKind::TarZst);
    }
    if name.ends_with(".zip") {
        return Some(ArchiveKind::Zip);
    }
    if name.ends_with(".7z") {
        return Some(ArchiveKind::SevenZ);
    }
    if name.ends_with(".rar") {
        return Some(ArchiveKind::Rar);
    }
    if name.ends_with(".tar") {
        return Some(ArchiveKind::Tar);
    }
    if name.ends_with(".gz") {
        return Some(ArchiveKind::Gz);
    }
    if name.ends_with(".bz2") {
        return Some(ArchiveKind::Bz2);
    }
    if name.ends_with(".xz") {
        return Some(ArchiveKind::Xz);
    }
    if name.ends_with(".zst") || name.ends_with(".zstd") {
        return Some(ArchiveKind::Zst);
    }
    None
}

/// Sniff magic bytes as a fallback when the extension is unknown.
fn sniff_by_magic(path: &Path) -> Option<ArchiveKind> {
    let mut file = File::open(path).ok()?;
    let mut buf = [0u8; 512];
    let n = file.read(&mut buf).ok()?;
    let b = &buf[..n];
    if b.starts_with(b"PK\x03\x04") || b.starts_with(b"PK\x05\x06") || b.starts_with(b"PK\x07\x08")
    {
        return Some(ArchiveKind::Zip);
    }
    if b.starts_with(b"7z\xBC\xAF'\x1C") {
        return Some(ArchiveKind::SevenZ);
    }
    if b.starts_with(b"Rar!\x1A\x07") {
        return Some(ArchiveKind::Rar);
    }
    if b.starts_with(b"\x1F\x8B") {
        return Some(ArchiveKind::Gz);
    }
    if b.starts_with(b"BZh") {
        return Some(ArchiveKind::Bz2);
    }
    if b.starts_with(b"\xFD7zXZ\x00") {
        return Some(ArchiveKind::Xz);
    }
    if b.starts_with(b"\x28\xB5\x2F\xFD") {
        return Some(ArchiveKind::Zst);
    }
    if b.len() > 262 && b[257..262] == *b"ustar" {
        return Some(ArchiveKind::Tar);
    }
    None
}

pub fn detect_archive(path: &Path) -> Option<ArchiveKind> {
    detect_by_ext(path).or_else(|| sniff_by_magic(path))
}

// ---------------------------------------------------------------------------
// Path safety
// ---------------------------------------------------------------------------

/// Rejects absolute paths and `..` traversal coming from archive entries.
fn sanitize_entry_path(entry: &Path) -> Result<PathBuf, String> {
    let mut out = PathBuf::new();
    for comp in entry.components() {
        match comp {
            Component::Normal(c) => out.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(format!("Unsafe path in archive: {}", entry.display()))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("Unsafe path in archive: {}", entry.display()))
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Analysis / listing
// ---------------------------------------------------------------------------

pub fn analyze_paths(paths: &[String]) -> Result<Vec<FileInfo>, String> {
    let mut out = Vec::with_capacity(paths.len());
    for p in paths {
        let path = PathBuf::from(p);
        let md = fs::metadata(&path).map_err(|e| format!("Cannot open {p}: {e}"))?;
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| p.clone());
        if md.is_dir() {
            let size = WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum();
            out.push(FileInfo {
                path: p.clone(),
                name,
                size,
                is_dir: true,
                kind: "folder".into(),
                archive_format: None,
            });
        } else {
            let fmt = detect_archive(&path).map(|k| k.label().to_string());
            out.push(FileInfo {
                path: p.clone(),
                name,
                size: md.len(),
                is_dir: false,
                kind: if fmt.is_some() { "archive" } else { "file" }.into(),
                archive_format: fmt,
            });
        }
    }
    Ok(out)
}

pub fn list_archive(path: &str) -> Result<Vec<ArchiveEntry>, String> {
    let path = Path::new(path);
    let kind = detect_archive(path)
        .ok_or_else(|| format!("Unsupported or unrecognized archive: {}", path.display()))?;
    match kind {
        ArchiveKind::Zip => list_zip(path),
        ArchiveKind::SevenZ => list_7z(path),
        ArchiveKind::Rar => list_rar(path),
        ArchiveKind::Tar
        | ArchiveKind::TarGz
        | ArchiveKind::TarBz2
        | ArchiveKind::TarXz
        | ArchiveKind::TarZst => list_tar(path),
        ArchiveKind::Gz | ArchiveKind::Bz2 | ArchiveKind::Xz | ArchiveKind::Zst => {
            let name = strip_single_ext(path).unwrap_or_else(|| path.display().to_string());
            Ok(vec![ArchiveEntry {
                path: name,
                size: 0,
                is_dir: false,
            }])
        }
    }
}

fn list_zip(path: &Path) -> Result<Vec<ArchiveEntry>, String> {
    let file = File::open(path).map_err(io_err)?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_err)?;
    let mut out = Vec::with_capacity(zip.len());
    for i in 0..zip.len() {
        let entry = zip.by_index(i).map_err(zip_err)?;
        out.push(ArchiveEntry {
            path: entry.name().to_string(),
            size: entry.size(),
            is_dir: entry.is_dir(),
        });
    }
    Ok(out)
}

fn list_7z(path: &Path) -> Result<Vec<ArchiveEntry>, String> {
    let reader = sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty())
        .map_err(|e| format!("{e}"))?;
    Ok(reader
        .archive()
        .files
        .iter()
        .map(|f| ArchiveEntry {
            path: f.name().to_string(),
            size: f.size(),
            is_dir: f.is_directory(),
        })
        .collect())
}

fn list_rar(path: &Path) -> Result<Vec<ArchiveEntry>, String> {
    let archive = unrar::Archive::new(path)
        .open_for_listing()
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in archive {
        let entry = entry.map_err(|e| e.to_string())?;
        out.push(ArchiveEntry {
            path: entry.filename.to_string_lossy().to_string(),
            size: entry.unpacked_size,
            is_dir: entry.is_directory(),
        });
    }
    Ok(out)
}

fn list_tar(path: &Path) -> Result<Vec<ArchiveEntry>, String> {
    let file = File::open(path).map_err(io_err)?;
    let reader = open_tar_reader(path, file)?;
    let mut tar = tar::Archive::new(reader);
    let mut out = Vec::new();
    for entry in tar.entries().map_err(io_err)? {
        let entry = entry.map_err(io_err)?;
        out.push(ArchiveEntry {
            path: entry.path().map_err(io_err)?.to_string_lossy().to_string(),
            size: entry.size(),
            is_dir: entry.header().entry_type().is_dir(),
        });
    }
    Ok(out)
}

fn strip_single_ext(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_string();
    for ext in [".gz", ".bz2", ".xz", ".zst", ".zstd"] {
        if name.to_ascii_lowercase().ends_with(ext) {
            return Some(name[..name.len() - ext.len()].to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

pub fn extract_archive(path: &Path, dest: &Path, progress: &mut Progress) -> Result<u64, String> {
    fs::create_dir_all(dest).map_err(io_err)?;
    let kind = detect_archive(path)
        .ok_or_else(|| format!("Unsupported or unrecognized archive: {}", path.display()))?;
    match kind {
        ArchiveKind::Zip => extract_zip(path, dest, progress),
        ArchiveKind::SevenZ => extract_7z(path, dest, progress),
        ArchiveKind::Rar => extract_rar(path, dest, progress),
        ArchiveKind::Tar
        | ArchiveKind::TarGz
        | ArchiveKind::TarBz2
        | ArchiveKind::TarXz
        | ArchiveKind::TarZst => extract_tar(path, dest, progress),
        ArchiveKind::Gz | ArchiveKind::Bz2 | ArchiveKind::Xz | ArchiveKind::Zst => {
            extract_single(path, dest, progress)
        }
    }
}

fn extract_zip(path: &Path, dest: &Path, progress: &mut Progress) -> Result<u64, String> {
    // Use file size as the total estimate to avoid a second pass.
    let total = fs::metadata(path).map_err(io_err)?.len();
    let file = File::open(path).map_err(io_err)?;
    let mut zip = zip::ZipArchive::new(file).map_err(zip_err)?;
    let mut current = 0u64;
    let mut count = 0u64;
    for i in 0..zip.len() {
        progress.check()?;
        let mut entry = zip.by_index(i).map_err(zip_err)?;
        let name = entry.name().to_string();
        let rel = entry
            .enclosed_name()
            .ok_or_else(|| format!("Unsafe path in archive: {name}"))?;
        let out = dest.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(io_err)?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(io_err)?;
            }
            let f = File::create(&out).map_err(io_err)?;
            let mut bw = BufWriter::new(f);
            let mut cr =
                CountingReader::new(&mut entry, progress, &mut current, total, "extract", name);
            io::copy(&mut cr, &mut bw).map_err(io_err)?;
            bw.flush().map_err(io_err)?;
            count += 1;
        }
    }
    Ok(count)
}

fn extract_7z(path: &Path, dest: &Path, progress: &mut Progress) -> Result<u64, String> {
    // Use compressed file size as total estimate to avoid a second pass.
    let total = fs::metadata(path).map_err(io_err)?.len();
    let mut reader = sevenz_rust::SevenZReader::open(path, sevenz_rust::Password::empty())
        .map_err(|e| format!("{e}"))?;
    let mut current = 0u64;
    let mut count = 0u64;
    let result = {
        let progress = &mut *progress;
        reader
            .for_each_entries(|entry, r| {
                if progress.cancel.load(Ordering::Relaxed) {
                    return Ok(false);
                }
                let name = entry.name().to_string();
                let rel = sanitize_entry_path(Path::new(&name))
                    .map_err(|e| sevenz_rust::Error::other(e))?;
                let out = dest.join(rel);
                if entry.is_directory() {
                    fs::create_dir_all(&out).map_err(|e| sevenz_rust::Error::io(e))?;
                } else {
                    if let Some(parent) = out.parent() {
                        fs::create_dir_all(parent).map_err(|e| sevenz_rust::Error::io(e))?;
                    }
                    let f = File::create(&out).map_err(|e| sevenz_rust::Error::io(e))?;
                    let mut bw = BufWriter::new(f);
                    let mut cr = CountingReader::new(
                        &mut *r,
                        progress,
                        &mut current,
                        total,
                        "extract",
                        name,
                    );
                    io::copy(&mut cr, &mut bw).map_err(|e| sevenz_rust::Error::io(e))?;
                    bw.flush().map_err(|e| sevenz_rust::Error::io(e))?;
                    count += 1;
                }
                Ok(true)
            })
            .map_err(|e| format!("{e}"))
    };
    if progress.cancel.load(Ordering::Relaxed) {
        return Err(CANCELLED.to_string());
    }
    result?;
    Ok(count)
}

fn extract_rar(path: &Path, dest: &Path, progress: &mut Progress) -> Result<u64, String> {
    // Use compressed file size as total estimate to avoid a second pass.
    let total = fs::metadata(path).map_err(io_err)?.len();
    let mut current = 0u64;
    let mut count = 0u64;
    let mut last_emit = std::time::Instant::now();

    let mut archive = unrar::Archive::new(path)
        .open_for_processing()
        .map_err(|e| e.to_string())?;
    loop {
        progress.check()?;
        let Some(header) = archive.read_header().map_err(|e| e.to_string())? else {
            break;
        };
        let (name, rel, is_dir, unpacked_size) = {
            let entry = header.entry();
            (
                entry.filename.to_string_lossy().to_string(),
                sanitize_entry_path(&entry.filename)?,
                entry.is_directory(),
                entry.unpacked_size,
            )
        };
        let out = dest.join(&rel);
        if is_dir {
            fs::create_dir_all(&out).map_err(io_err)?;
            archive = header.skip().map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(io_err)?;
            }
            archive = header.extract_to(&out).map_err(|e| e.to_string())?;
            count += 1;
        }
        current += unpacked_size;
        // Throttle progress to ~100 ms.
        if last_emit.elapsed().as_millis() >= 100 || current >= total {
            progress.emit("extract", current, total, &name);
            last_emit = std::time::Instant::now();
        }
    }
    Ok(count)
}

fn open_tar_reader(path: &Path, file: File) -> Result<Box<dyn Read>, String> {
    let kind = detect_archive(path)
        .ok_or_else(|| format!("Unsupported or unrecognized archive: {}", path.display()))?;
    match kind {
        ArchiveKind::Tar => Ok(Box::new(file)),
        ArchiveKind::TarGz => Ok(Box::new(flate2::read::GzDecoder::new(file))),
        ArchiveKind::TarBz2 => Ok(Box::new(bzip2::read::BzDecoder::new(file))),
        ArchiveKind::TarXz => Ok(Box::new(xz2::read::XzDecoder::new(file))),
        ArchiveKind::TarZst => Ok(Box::new(
            zstd::stream::read::Decoder::new(file).map_err(io_err)?,
        )),
        _ => Err(format!("Not a tar archive: {}", path.display())),
    }
}

fn extract_tar(path: &Path, dest: &Path, progress: &mut Progress) -> Result<u64, String> {
    // Use compressed file size as the total estimate to avoid a second pass.
    let total = fs::metadata(path).map_err(io_err)?.len();
    let file = File::open(path).map_err(io_err)?;
    let reader = open_tar_reader(path, file)?;
    let mut tar = tar::Archive::new(reader);
    let mut current = 0u64;
    let mut count = 0u64;
    for entry in tar.entries().map_err(io_err)? {
        progress.check()?;
        let mut entry = entry.map_err(io_err)?;
        let p = entry.path().map_err(io_err)?;
        let name = p.to_string_lossy().to_string();
        let rel = sanitize_entry_path(&p)?;
        let out = dest.join(rel);
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&out).map_err(io_err)?;
        } else {
            if let Some(parent) = out.parent() {
                fs::create_dir_all(parent).map_err(io_err)?;
            }
            let f = File::create(&out).map_err(io_err)?;
            let mut bw = BufWriter::new(f);
            let mut cr =
                CountingReader::new(&mut entry, progress, &mut current, total, "extract", name);
            io::copy(&mut cr, &mut bw).map_err(io_err)?;
            bw.flush().map_err(io_err)?;
            count += 1;
        }
    }
    Ok(count)
}

fn extract_single(path: &Path, dest: &Path, progress: &mut Progress) -> Result<u64, String> {
    let kind = detect_archive(path)
        .ok_or_else(|| format!("Unsupported or unrecognized archive: {}", path.display()))?;
    let file = File::open(path).map_err(io_err)?;
    let total = fs::metadata(path).map_err(io_err)?.len();
    let mut current = 0u64;
    let mut last_emit = std::time::Instant::now();
    let mut reader: Box<dyn Read> = match kind {
        ArchiveKind::Gz => Box::new(BufReader::new(flate2::read::GzDecoder::new(file))),
        ArchiveKind::Bz2 => Box::new(BufReader::new(bzip2::read::BzDecoder::new(file))),
        ArchiveKind::Xz => Box::new(BufReader::new(xz2::read::XzDecoder::new(file))),
        ArchiveKind::Zst => Box::new(BufReader::new(
            zstd::stream::read::Decoder::new(file).map_err(io_err)?,
        )),
        _ => return Err("Not a single-file compressed stream".into()),
    };
    let out_name = strip_single_ext(path).unwrap_or_else(|| "output".to_string());
    let out = dest.join(&out_name);
    let f = File::create(&out).map_err(io_err)?;
    let mut bw = BufWriter::with_capacity(1024 * 1024, f);
    let mut buf = [0u8; 1024 * 1024];
    loop {
        progress.check()?;
        let n = reader.read(&mut buf).map_err(io_err)?;
        if n == 0 {
            break;
        }
        bw.write_all(&buf[..n]).map_err(io_err)?;
        current += n as u64;
        if last_emit.elapsed().as_millis() >= 100 || current >= total {
            progress.emit("extract", current, total, &out_name);
            last_emit = std::time::Instant::now();
        }
    }
    bw.flush().map_err(io_err)?;
    progress.emit("extract", current, total, &out_name);
    Ok(1)
}

// ---------------------------------------------------------------------------
// Compression
// ---------------------------------------------------------------------------

struct RootEntry {
    path: PathBuf,
    name: String,
}

/// Recursively collect (path, archive-relative name, is_dir) for a root entry.
/// The root itself is included so folders keep their name inside the archive.
fn collect_entries(root: &Path, root_name: &str) -> Vec<(PathBuf, String, bool)> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path().to_path_buf();
        let rel = path.strip_prefix(root).unwrap_or(entry.path());
        let rel_str = if rel.as_os_str().is_empty() {
            root_name.to_string()
        } else {
            let sub = rel.to_string_lossy().replace('\\', "/");
            format!("{root_name}/{sub}")
        };
        out.push((path, rel_str, entry.file_type().is_dir()));
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

fn level_of(level: &str, fast: i64, balanced: i64, maximum: i64) -> i64 {
    match level {
        "fast" => fast,
        "maximum" => maximum,
        _ => balanced,
    }
}

/// Compress a set of files/folders/archives into a new archive at `output`.
///
/// Archive inputs are first extracted into a temporary staging directory
/// ("re-compress"), then everything is written with the requested format.
pub fn compress_files(
    paths: &[String],
    output: &str,
    format: &str,
    level: &str,
    progress: &mut Progress,
) -> Result<CompressResult, String> {
    let started = Instant::now();
    let out = PathBuf::from(output);

    // Media formats (H.265 video / AVIF image) are transcoded via ffmpeg,
    // not packed into an archive.
    if format == "h265" || format == "avif" {
        return crate::media::compress_media(paths, output, format, level, progress);
    }

    // Guard against overwriting an input file.
    let out_abs = if out.is_absolute() {
        out.clone()
    } else {
        std::env::current_dir().map_err(io_err)?.join(&out)
    };
    let out_canon = fs::canonicalize(&out_abs).unwrap_or(out_abs);
    for p in paths {
        let pp = PathBuf::from(p);
        if pp.exists() {
            if let Ok(c) = fs::canonicalize(&pp) {
                if c == out_canon {
                    return Err("The output file must be different from the input files.".into());
                }
            }
        }
    }
    if out.exists() {
        fs::remove_file(&out).map_err(io_err)?;
    }
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }

    let tmp = tempfile::tempdir().map_err(io_err)?;

    // Build the list of roots; archives are staged into the temp dir.
    let mut roots: Vec<RootEntry> = Vec::new();
    for (i, p) in paths.iter().enumerate() {
        progress.check()?;
        let path = PathBuf::from(p);
        if !path.exists() {
            return Err(format!("Path does not exist: {p}"));
        }
        if path.is_dir() {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("folder_{i}"));
            roots.push(RootEntry { path, name });
        } else if detect_archive(&path).is_some() {
            let stem = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("archive_{i}"));
            let stage = tmp.path().join(format!("{i}_{stem}"));
            fs::create_dir_all(&stage).map_err(io_err)?;
            progress.emit("extract", 0, 0, &format!("Opening {stem}…"));
            extract_archive(&path, &stage, progress)?;
            roots.push(RootEntry {
                path: stage,
                name: stem,
            });
        } else {
            let name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("file_{i}"));
            roots.push(RootEntry { path, name });
        }
    }

    // Collect all entries and total uncompressed size.
    let mut entries: Vec<(PathBuf, String, bool)> = Vec::new();
    let mut total = 0u64;
    for root in &roots {
        for (path, rel, is_dir) in collect_entries(&root.path, &root.name) {
            if !is_dir {
                total += fs::metadata(&path).map_err(io_err)?.len();
            }
            entries.push((path, rel, is_dir));
        }
    }

    progress.emit("compress", 0, total, "Starting…");

    let count = match format {
        "zip" => write_zip(&out, &entries, level_of(level, 1, 6, 9), progress, total)?,
        "7z" => write_7z(&out, &entries, progress, total)?,
        "tgz" => write_tar(
            &out,
            &entries,
            "tgz",
            level_of(level, 1, 6, 9),
            progress,
            total,
        )?,
        "tzst" => write_tar(
            &out,
            &entries,
            "tzst",
            level_of(level, 3, 9, 19),
            progress,
            total,
        )?,
        _ => return Err(format!("Unknown output format: {format}")),
    };

    let output_size = fs::metadata(&out).map_err(io_err)?.len();
    Ok(CompressResult {
        output: out.to_string_lossy().to_string(),
        output_size,
        original_size: total,
        entries: count,
        duration_ms: started.elapsed().as_millis() as u64,
    })
}

fn write_zip(
    out: &Path,
    entries: &[(PathBuf, String, bool)],
    level: i64,
    progress: &mut Progress,
    total: u64,
) -> Result<u64, String> {
    let file = File::create(out).map_err(io_err)?;
    let mut zip = zip::ZipWriter::new(BufWriter::with_capacity(1024 * 1024, file));
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .compression_level(Some(level));
    let mut current = 0u64;
    for (path, name, is_dir) in entries {
        progress.check()?;
        if *is_dir {
            zip.add_directory(name.as_str(), opts).map_err(zip_err)?;
        } else {
            zip.start_file(name.as_str(), opts).map_err(zip_err)?;
            let f = File::open(path).map_err(io_err)?;
            let mut br = BufReader::with_capacity(1024 * 1024, f);
            let mut cr =
                CountingReader::new(&mut br, progress, &mut current, total, "compress", name.clone());
            io::copy(&mut cr, &mut zip).map_err(io_err)?;
        }
    }
    zip.finish().map_err(zip_err)?;
    Ok(entries.len() as u64)
}

fn write_7z(
    out: &Path,
    entries: &[(PathBuf, String, bool)],
    progress: &mut Progress,
    total: u64,
) -> Result<u64, String> {
    let mut sz = sevenz_rust::SevenZWriter::create(out).map_err(|e| format!("{e}"))?;
    let mut current = 0u64;
    for (path, name, is_dir) in entries {
        progress.check()?;
        let entry = sevenz_rust::SevenZArchiveEntry::from_path(path, name.clone());
        if *is_dir {
            sz.push_archive_entry(entry, None::<std::io::Empty>)
                .map_err(|e| format!("{e}"))?;
        } else {
            let f = File::open(path).map_err(io_err)?;
            let mut br = BufReader::with_capacity(1024 * 1024, f);
            let cr =
                CountingReader::new(&mut br, progress, &mut current, total, "compress", name.clone());
            sz.push_archive_entry(entry, Some(cr))
                .map_err(|e| format!("{e}"))?;
        }
    }
    sz.finish().map_err(|e| format!("{e}"))?;
    Ok(entries.len() as u64)
}

fn append_tar_entries<W: Write>(
    tar: &mut tar::Builder<W>,
    entries: &[(PathBuf, String, bool)],
    progress: &mut Progress,
    current: &mut u64,
    total: u64,
) -> Result<(), String> {
    for (path, name, is_dir) in entries {
        progress.check()?;
        if *is_dir {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Directory);
            header.set_size(0);
            header.set_mode(0o755);
            if let Ok(md) = fs::metadata(path) {
                header.set_metadata(&md);
            }
            header.set_cksum();
            tar.append_data(&mut header, name, io::empty())
                .map_err(io_err)?;
        } else {
            let md = fs::metadata(path).map_err(io_err)?;
            let mut header = tar::Header::new_gnu();
            header.set_metadata(&md);
            header.set_size(md.len());
            header.set_cksum();
            let f = File::open(path).map_err(io_err)?;
            let mut br = BufReader::with_capacity(1024 * 1024, f);
            let cr = CountingReader::new(&mut br, progress, current, total, "compress", name.clone());
            tar.append_data(&mut header, name, cr).map_err(io_err)?;
        }
    }
    Ok(())
}

fn write_tar(
    out: &Path,
    entries: &[(PathBuf, String, bool)],
    format: &str,
    level: i64,
    progress: &mut Progress,
    total: u64,
) -> Result<u64, String> {
    let mut current = 0u64;
    match format {
        "tgz" => {
            let file = File::create(out).map_err(io_err)?;
            let mut enc =
                flate2::write::GzEncoder::new(BufWriter::with_capacity(1024 * 1024, file), flate2::Compression::new(level as u32));
            let mut tar = tar::Builder::new(&mut enc);
            append_tar_entries(&mut tar, entries, progress, &mut current, total)?;
            let _ = tar.into_inner().map_err(io_err)?;
            enc.finish().map_err(io_err)?;
        }
        "tzst" => {
            let file = File::create(out).map_err(io_err)?;
            let mut enc = zstd::stream::write::Encoder::new(BufWriter::with_capacity(1024 * 1024, file), level as i32).map_err(io_err)?;
            let threads = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1) as u32;
            enc.multithread(threads).map_err(io_err)?;
            let mut tar = tar::Builder::new(&mut enc);
            append_tar_entries(&mut tar, entries, progress, &mut current, total)?;
            let _ = tar.into_inner().map_err(io_err)?;
            enc.finish().map_err(io_err)?;
        }
        _ => return Err(format!("Unknown tar format: {format}")),
    }
    Ok(entries.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noop_progress() -> Progress {
        Progress {
            cancel: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            emit: Box::new(|_, _, _, _| {}),
        }
    }

    fn setup_src(dir: &Path) {
        let sub = dir.join("sub");
        fs::create_dir_all(&sub).unwrap();
        // Repetitive text (compresses well)
        let text = "FilePressor compression test line\n".repeat(5_000);
        fs::write(dir.join("hello.txt"), text).unwrap();
        // Random-ish binary data
        let mut rng_bytes = Vec::with_capacity(256 * 1024);
        for i in 0..256usize * 1024 {
            rng_bytes.push(((i * 31 + 7) % 251) as u8);
        }
        fs::write(sub.join("blob.bin"), rng_bytes).unwrap();
        fs::write(sub.join("empty.dat"), b"").unwrap();
    }

    /// Archives keep the root folder name (7-Zip style), so the extracted
    /// output contains exactly one top-level folder whose contents must match.
    fn assert_tree_equal(src: &Path, dest: &Path) {
        let tops: Vec<_> = fs::read_dir(dest).unwrap().filter_map(|e| e.ok()).collect();
        assert_eq!(
            tops.len(),
            1,
            "expected one root folder in dest, got {tops:?}"
        );
        let root = tops[0].path();
        assert!(root.is_dir(), "root {root:?} should be a directory");
        for entry in WalkDir::new(src) {
            let entry = entry.unwrap();
            let rel = entry.path().strip_prefix(src).unwrap();
            let other = root.join(rel);
            assert!(other.exists(), "missing {rel:?} in extracted output");
            if entry.file_type().is_file() {
                assert_eq!(
                    fs::read(entry.path()).unwrap(),
                    fs::read(&other).unwrap(),
                    "content mismatch for {rel:?}"
                );
            }
        }
    }

    #[test]
    fn compress_and_extract_all_formats() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        setup_src(&src);

        for fmt in ["zip", "7z", "tgz", "tzst"] {
            let out = tmp.path().join(format!("out.{fmt}"));
            let result = compress_files(
                &[src.to_string_lossy().to_string()],
                out.to_str().unwrap(),
                fmt,
                "balanced",
                &mut noop_progress(),
            )
            .expect(fmt);
            assert!(fs::metadata(&out).unwrap().len() > 0);
            assert!(
                result.output_size < result.original_size,
                "{fmt} should shrink"
            );

            let dest = tmp.path().join(format!("out_extracted_{fmt}"));
            extract_archive(&out, &dest, &mut noop_progress()).expect(fmt);
            assert_tree_equal(&src, &dest);
        }
    }

    #[test]
    fn recompress_archive_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        setup_src(&src);

        // First make a zip
        let zip_path = tmp.path().join("first.zip");
        compress_files(
            &[src.to_string_lossy().to_string()],
            zip_path.to_str().unwrap(),
            "zip",
            "balanced",
            &mut noop_progress(),
        )
        .unwrap();

        // Re-compress the zip into a 7z (exercises the staging path)
        let seven = tmp.path().join("second.7z");
        compress_files(
            &[zip_path.to_string_lossy().to_string()],
            seven.to_str().unwrap(),
            "7z",
            "balanced",
            &mut noop_progress(),
        )
        .unwrap();

        let dest = tmp.path().join("re_extracted");
        extract_archive(&seven, &dest, &mut noop_progress()).unwrap();
        // Re-compressed archives keep their internal layout, so locate each
        // original file anywhere under the extracted tree and compare bytes.
        for name in ["hello.txt", "blob.bin", "empty.dat"] {
            let found = WalkDir::new(&dest)
                .into_iter()
                .filter_map(|e| e.ok())
                .find(|e| e.file_type().is_file() && e.file_name() == name)
                .unwrap_or_else(|| panic!("missing {name} under {dest:?}"));
            let original = src.join(match name {
                "blob.bin" | "empty.dat" => format!("sub/{name}"),
                _ => name.to_string(),
            });
            assert_eq!(
                fs::read(found.path()).unwrap_or_else(|e| panic!("read found {found:?}: {e}")),
                fs::read(&original)
                    .unwrap_or_else(|e| panic!("read original {original:?} from {src:?}: {e}")),
                "content mismatch for {name}"
            );
        }
    }

    #[test]
    fn single_file_streams() {
        let tmp = tempfile::tempdir().unwrap();
        let data = "hello gzip bzip2 xz zstd\n".repeat(1000);
        let plain = tmp.path().join("plain.txt");
        fs::write(&plain, &data).unwrap();

        // Build each single-file format by hand and decompress via extract_archive
        let gz = tmp.path().join("plain.txt.gz");
        let mut enc = flate2::write::GzEncoder::new(
            File::create(&gz).unwrap(),
            flate2::Compression::default(),
        );
        enc.write_all(data.as_bytes()).unwrap();
        enc.finish().unwrap();

        let bz2 = tmp.path().join("plain.txt.bz2");
        {
            let f = File::create(&bz2).unwrap();
            let mut enc = bzip2::write::BzEncoder::new(f, bzip2::Compression::default());
            enc.write_all(data.as_bytes()).unwrap();
            enc.finish().unwrap();
        }

        let xz = tmp.path().join("plain.txt.xz");
        {
            let f = File::create(&xz).unwrap();
            let mut enc = xz2::write::XzEncoder::new(f, 6);
            enc.write_all(data.as_bytes()).unwrap();
            enc.finish().unwrap();
        }

        let zst = tmp.path().join("plain.txt.zst");
        {
            let f = File::create(&zst).unwrap();
            let mut enc = zstd::stream::write::Encoder::new(f, 3).unwrap();
            enc.write_all(data.as_bytes()).unwrap();
            enc.finish().unwrap();
        }

        for p in [&gz, &bz2, &xz, &zst] {
            let dest = tmp
                .path()
                .join(format!("d_{}", p.file_name().unwrap().to_string_lossy()));
            fs::create_dir_all(&dest).unwrap();
            extract_archive(p, &dest, &mut noop_progress()).unwrap();
            assert_eq!(fs::read(dest.join("plain.txt")).unwrap(), data.as_bytes());
        }
    }
}
