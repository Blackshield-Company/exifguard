use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use exifguard::report::{format_check_report, format_scan_table, AnalysisReport};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(
    name = "exifguard",
    version,
    about = "Batch forensic verifier for evidence photos — flags signs of editing or tampering via EXIF metadata analysis",
    long_about = "exifguard scans photos for forensic red flags in their EXIF metadata: \
stripped metadata, editing-software tags, impossible timestamps, missing camera identity, \
and stale embedded thumbnails. Flags are investigative leads, not proof. Local-first: no \
network access, nothing leaves your machine."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Recursively scan a directory of JPEG files and print a report table.
    Scan {
        /// Directory to scan (searched recursively for .jpg/.jpeg files).
        dir: PathBuf,
        /// Emit machine-readable JSON instead of a table.
        #[arg(long)]
        json: bool,
    },
    /// Print a detailed forensic report for a single file.
    Check {
        /// Image file to analyze.
        file: PathBuf,
        /// Emit machine-readable JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Scan { dir, json } => scan(&dir, json),
        Commands::Check { file, json } => check(&file, json),
    }
}

fn scan(dir: &Path, json: bool) -> Result<()> {
    anyhow::ensure!(dir.is_dir(), "not a directory: {}", dir.display());

    let mut reports: Vec<AnalysisReport> = Vec::new();
    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || !is_jpeg(path) {
            continue;
        }
        match exifguard::analyze_file(path) {
            Ok(r) => reports.push(r),
            Err(e) => eprintln!("warning: {}: {e:#}", path.display()),
        }
    }
    reports.sort_by(|a, b| a.path.cmp(&b.path));

    if json {
        println!("{}", serde_json::to_string_pretty(&reports)?);
    } else {
        print!("{}", format_scan_table(&reports));
    }
    Ok(())
}

fn check(file: &Path, json: bool) -> Result<()> {
    anyhow::ensure!(file.is_file(), "not a file: {}", file.display());
    let report = exifguard::analyze_file(file)
        .with_context(|| format!("failed to analyze {}", file.display()))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print!("{}", format_check_report(&report));
    }
    Ok(())
}

fn is_jpeg(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("jpg") || e.eq_ignore_ascii_case("jpeg"))
        .unwrap_or(false)
}
