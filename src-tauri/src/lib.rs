//! Exifguard desktop. Same checks as the CLI. Nothing leaves the machine.

use exifguard::report::{format_check_report, format_scan_table, AnalysisReport};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
struct CheckResult {
    report: AnalysisReport,
    text: String,
}

#[derive(Serialize)]
struct ScanError {
    path: String,
    message: String,
}

#[derive(Serialize)]
struct ScanResult {
    reports: Vec<AnalysisReport>,
    text: String,
    errors: Vec<ScanError>,
}

fn nonempty(path: &str) -> Result<&str, String> {
    let path = path.trim();
    if path.is_empty() {
        Err("empty path".into())
    } else {
        Ok(path)
    }
}

fn is_jpeg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
        .unwrap_or(false)
}

/// Recursive walk matching the CLI: follow links, skip unreadable entries,
/// keep `.jpg` / `.jpeg` only. Cycles are skipped. Analyze failures are not
/// collected here — the caller turns those into error rows.
fn walk_jpegs(dir: &Path, seen: &mut HashSet<PathBuf>, out: &mut Vec<PathBuf>) {
    let key = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !seen.insert(key) {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let meta = match fs::metadata(&path) {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        if meta.is_dir() {
            walk_jpegs(&path, seen, out);
        } else if meta.is_file() && is_jpeg(&path) {
            out.push(path);
        }
    }
}

#[tauri::command]
fn check_file(path: String) -> Result<CheckResult, String> {
    let path = nonempty(&path)?;
    let file = Path::new(path);
    if !file.exists() {
        return Err(format!("not found: {path}"));
    }
    if !file.is_file() {
        return Err(format!("not a file: {path}"));
    }
    let report = exifguard::analyze_file(file)
        .map_err(|e| format!("failed to analyze {}: {e:#}", file.display()))?;
    let text = format_check_report(&report);
    Ok(CheckResult { report, text })
}

#[tauri::command]
fn scan_dir(path: String) -> Result<ScanResult, String> {
    let path = nonempty(&path)?;
    let dir = Path::new(path);
    if !dir.exists() {
        return Err(format!("not found: {path}"));
    }
    if !dir.is_dir() {
        return Err(format!("not a directory: {path}"));
    }

    let mut files = Vec::new();
    walk_jpegs(dir, &mut HashSet::new(), &mut files);
    files.sort();

    let mut reports = Vec::new();
    let mut errors = Vec::new();
    for file in files {
        match exifguard::analyze_file(&file) {
            Ok(report) => reports.push(report),
            Err(e) => errors.push(ScanError {
                path: file.display().to_string(),
                message: format!("{e:#}"),
            }),
        }
    }
    reports.sort_by(|a, b| a.path.cmp(&b.path));
    let text = format_scan_table(&reports);
    Ok(ScanResult {
        reports,
        text,
        errors,
    })
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![check_file, scan_dir])
        .run(tauri::generate_context!())
        .expect("error while running exifguard");
}
