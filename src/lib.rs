//! exifguard — batch forensic verifier for evidence photos.
//!
//! Analyzes EXIF metadata for signs of editing or tampering. Flags are
//! investigative leads, not proof of manipulation.

pub mod exif_extract;
pub mod flags;
pub mod report;
pub mod tags;

pub use exif_extract::analyze_file;
pub use flags::{detect_flags, Finding};
pub use report::AnalysisReport;
pub use tags::TagData;
