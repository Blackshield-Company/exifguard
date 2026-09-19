//! End-to-end tests: build synthetic JPEG fixtures with crafted EXIF in
//! test setup (no real camera files needed) and run the actual binary.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Build a TIFF block with the given ASCII fields using kamadak-exif's writer.
fn build_tiff(fields: &[exif::Field]) -> Vec<u8> {
    let mut w = exif::experimental::Writer::new();
    for f in fields {
        w.push_field(f);
    }
    let mut buf = Cursor::new(Vec::new());
    w.write(&mut buf, false).expect("write tiff");
    buf.into_inner()
}

fn ascii(s: &str) -> exif::Value {
    let mut v = s.as_bytes().to_vec();
    v.push(0);
    exif::Value::Ascii(vec![v])
}

fn primary(tag: exif::Tag, value: exif::Value) -> exif::Field {
    exif::Field {
        ifd_num: exif::In::PRIMARY,
        tag,
        value,
    }
}

fn thumbnail(tag: exif::Tag, value: exif::Value) -> exif::Field {
    exif::Field {
        ifd_num: exif::In::THUMBNAIL,
        tag,
        value,
    }
}

/// Wrap TIFF bytes into a minimal-but-parseable JPEG container:
/// SOI, APP1(Exif), SOF0 (carrying the main image dimensions), EOI.
fn build_jpeg(tiff: Option<&[u8]>, width: u16, height: u16) -> Vec<u8> {
    let mut jpg = vec![0xFF, 0xD8]; // SOI
    if let Some(t) = tiff {
        let payload_len = 6 + t.len();
        let seg_len = (payload_len + 2) as u16;
        jpg.extend_from_slice(&[0xFF, 0xE1]);
        jpg.extend_from_slice(&seg_len.to_be_bytes());
        jpg.extend_from_slice(b"Exif\0\0");
        jpg.extend_from_slice(t);
    }
    // SOF0
    jpg.extend_from_slice(&[0xFF, 0xC0]);
    jpg.extend_from_slice(&17u16.to_be_bytes());
    jpg.push(8); // precision
    jpg.extend_from_slice(&height.to_be_bytes());
    jpg.extend_from_slice(&width.to_be_bytes());
    jpg.push(3); // components
    jpg.extend_from_slice(&[1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    jpg.extend_from_slice(&[0xFF, 0xD9]); // EOI
    jpg
}

struct FixtureDir {
    path: PathBuf,
}

impl FixtureDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("exifguard-test-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        FixtureDir { path }
    }

    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path.join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, bytes).unwrap();
        p
    }
}

impl Drop for FixtureDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn run(args: &[&str]) -> (String, String, bool) {
    let out = Command::new(env!("CARGO_BIN_EXE_exifguard"))
        .args(args)
        .output()
        .expect("run exifguard");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.success(),
    )
}

fn check_json(path: &Path) -> serde_json::Value {
    let (stdout, stderr, ok) = run(&["check", path.to_str().unwrap(), "--json"]);
    assert!(ok, "check failed: {stderr}");
    serde_json::from_str(&stdout).expect("valid JSON from check --json")
}

fn flag_codes(report: &serde_json::Value) -> Vec<String> {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn edited_photo_flags_software_tag() {
    let dir = FixtureDir::new("edited");
    let tiff = build_tiff(&[
        primary(exif::Tag::Make, ascii("Canon")),
        primary(exif::Tag::Model, ascii("Canon EOS R5")),
        primary(exif::Tag::Software, ascii("GIMP 2.10.38")),
        primary(exif::Tag::DateTimeOriginal, ascii("2024:03:10 14:22:01")),
    ]);
    let jpg = build_jpeg(Some(&tiff), 8192, 5464);
    let p = dir.write("edited.jpg", &jpg);

    let report = check_json(&p);
    let codes = flag_codes(&report);
    assert!(
        codes.contains(&"SOFTWARE_TAG".to_string()),
        "expected SoftwareTag, got {codes:?}"
    );
    assert_eq!(report["tags"]["software"], "GIMP 2.10.38");
    // Explanation must be human-readable, not just a code.
    let explanation = report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["code"] == "SOFTWARE_TAG")
        .unwrap()["explanation"]
        .as_str()
        .unwrap();
    assert!(explanation.contains("Software tag"));
}

#[test]
fn exifless_photo_flags_no_exif() {
    let dir = FixtureDir::new("stripped");
    let jpg = build_jpeg(None, 4000, 3000); // no APP1 segment at all
    let p = dir.write("stripped.jpeg", &jpg);

    let report = check_json(&p);
    assert_eq!(flag_codes(&report), vec!["NO_EXIF".to_string()]);
    assert_eq!(report["tags"]["has_exif"], false);
    assert_eq!(
        report["tags"]["image_dimensions"],
        serde_json::json!([4000, 3000])
    );
}

#[test]
fn missing_camera_and_datetime_are_flagged() {
    let dir = FixtureDir::new("nocam");
    let tiff = build_tiff(&[primary(exif::Tag::ImageDescription, ascii("some image"))]);
    let jpg = build_jpeg(Some(&tiff), 2000, 1000);
    let p = dir.write("nocam.jpg", &jpg);

    let report = check_json(&p);
    let codes = flag_codes(&report);
    assert!(
        codes.contains(&"MISSING_CAMERA_INFO".to_string()),
        "{codes:?}"
    );
    assert!(
        codes.contains(&"MISSING_DATETIME_ORIGINAL".to_string()),
        "{codes:?}"
    );
}

#[test]
fn forged_timestamp_later_than_mtime_is_flagged() {
    let dir = FixtureDir::new("forged");
    // Claim the photo was taken in the year 2100 — after this file's mtime.
    let tiff = build_tiff(&[
        primary(exif::Tag::Make, ascii("TestCam")),
        primary(exif::Tag::Model, ascii("Model X")),
        primary(exif::Tag::DateTimeOriginal, ascii("2100:01:01 00:00:01")),
    ]);
    let jpg = build_jpeg(Some(&tiff), 2000, 1000);
    let p = dir.write("forged.jpg", &jpg);

    let report = check_json(&p);
    let codes = flag_codes(&report);
    assert!(
        codes.contains(&"DATETIME_MTIME_MISMATCH".to_string()),
        "{codes:?}"
    );
}

#[test]
fn thumbnail_aspect_mismatch_is_flagged() {
    let dir = FixtureDir::new("thumb");
    let tiff = build_tiff(&[
        primary(exif::Tag::Make, ascii("TestCam")),
        primary(exif::Tag::Model, ascii("Model X")),
        primary(exif::Tag::DateTimeOriginal, ascii("2024:03:10 14:22:01")),
        thumbnail(exif::Tag::PixelXDimension, exif::Value::Long(vec![160])),
        thumbnail(exif::Tag::PixelYDimension, exif::Value::Long(vec![160])), // square!
        thumbnail(
            exif::Tag::JPEGInterchangeFormatLength,
            exif::Value::Long(vec![100]),
        ),
    ]);
    let jpg = build_jpeg(Some(&tiff), 4000, 3000); // 4:3 main image
    let p = dir.write("thumb.jpg", &jpg);

    let report = check_json(&p);
    let codes = flag_codes(&report);
    assert!(
        codes.contains(&"THUMBNAIL_MISMATCH".to_string()),
        "{codes:?}"
    );
}

#[test]
fn plausible_original_is_clean() {
    let dir = FixtureDir::new("clean");
    let tiff = build_tiff(&[
        primary(exif::Tag::Make, ascii("TestCam")),
        primary(exif::Tag::Model, ascii("Model X")),
        primary(exif::Tag::DateTimeOriginal, ascii("2024:03:10 14:22:01")),
        thumbnail(exif::Tag::PixelXDimension, exif::Value::Long(vec![160])),
        thumbnail(exif::Tag::PixelYDimension, exif::Value::Long(vec![120])),
        thumbnail(
            exif::Tag::JPEGInterchangeFormatLength,
            exif::Value::Long(vec![100]),
        ),
    ]);
    let jpg = build_jpeg(Some(&tiff), 4000, 3000);
    let p = dir.write("clean.jpg", &jpg);

    let report = check_json(&p);
    assert_eq!(flag_codes(&report), Vec::<String>::new());
    assert_eq!(report["clean"], true);
}

#[test]
fn scan_recurses_and_reports_json_array() {
    let dir = FixtureDir::new("scan");
    let edited_tiff = build_tiff(&[
        primary(exif::Tag::Make, ascii("TestCam")),
        primary(exif::Tag::Model, ascii("Model X")),
        primary(exif::Tag::Software, ascii("Adobe Photoshop 25.0 (Windows)")),
        primary(exif::Tag::DateTimeOriginal, ascii("2024:03:10 14:22:01")),
    ]);
    dir.write(
        "nested/deep/edited.jpg",
        &build_jpeg(Some(&edited_tiff), 1000, 800),
    );
    dir.write("top.jpg", &build_jpeg(None, 1000, 800));
    dir.write("ignore.txt", b"not a jpeg"); // must be ignored
    dir.write("also.png", b"fake png"); // must be ignored

    let (stdout, stderr, ok) = run(&["scan", dir.path.to_str().unwrap(), "--json"]);
    assert!(ok, "scan failed: {stderr}");
    let reports: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON array");
    let arr = reports.as_array().unwrap();
    assert_eq!(arr.len(), 2, "expected exactly 2 JPEGs, got {arr:?}");

    let by_name = |needle: &str| {
        arr.iter()
            .find(|r| r["path"].as_str().unwrap().contains(needle))
            .unwrap_or_else(|| panic!("missing report for {needle}"))
    };
    let edited = by_name("edited.jpg");
    assert!(
        flag_codes(edited).contains(&"SOFTWARE_TAG".to_string()),
        "{edited:?}"
    );
    let top = by_name("top.jpg");
    assert_eq!(flag_codes(top), vec!["NO_EXIF".to_string()]);
}

#[test]
fn scan_table_output_mentions_flags_and_totals() {
    let dir = FixtureDir::new("table");
    dir.write("a.jpg", &build_jpeg(None, 640, 480));
    let (stdout, stderr, ok) = run(&["scan", dir.path.to_str().unwrap()]);
    assert!(ok, "scan failed: {stderr}");
    assert!(stdout.contains("PATH"), "{stdout}");
    assert!(stdout.contains("NO_EXIF"), "{stdout}");
    assert!(stdout.contains("1 file(s) scanned, 1 flagged"), "{stdout}");
}

#[test]
fn check_rejects_missing_file() {
    let (_, _, ok) = run(&["check", "/nonexistent/photo.jpg"]);
    assert!(!ok);
}
