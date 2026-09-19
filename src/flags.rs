use crate::tags::TagData;
use serde::Serialize;
use std::fmt;

/// Stable machine-readable codes for each finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum FlagCode {
    #[serde(rename = "NO_EXIF")]
    NoExif,
    #[serde(rename = "SOFTWARE_TAG")]
    SoftwareTag,
    #[serde(rename = "MISSING_DATETIME_ORIGINAL")]
    MissingDateTimeOriginal,
    #[serde(rename = "DATETIME_MTIME_MISMATCH")]
    DateTimeMtimeMismatch,
    #[serde(rename = "MISSING_CAMERA_INFO")]
    MissingCameraInfo,
    #[serde(rename = "THUMBNAIL_MISMATCH")]
    ThumbnailMismatch,
}

impl fmt::Display for FlagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            FlagCode::NoExif => "NO_EXIF",
            FlagCode::SoftwareTag => "SOFTWARE_TAG",
            FlagCode::MissingDateTimeOriginal => "MISSING_DATETIME_ORIGINAL",
            FlagCode::DateTimeMtimeMismatch => "DATETIME_MTIME_MISMATCH",
            FlagCode::MissingCameraInfo => "MISSING_CAMERA_INFO",
            FlagCode::ThumbnailMismatch => "THUMBNAIL_MISMATCH",
        };
        f.write_str(s)
    }
}

/// A single forensic finding with a human-readable explanation.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub code: FlagCode,
    /// Short headline.
    pub title: String,
    /// Why this matters, in plain language.
    pub explanation: String,
    /// Case-specific detail (tag values, dimensions, etc.).
    pub detail: String,
}

/// Run all flag-detection rules against normalized tag data.
///
/// Pure function of `TagData` — unit-testable without real image files.
pub fn detect_flags(t: &TagData) -> Vec<Finding> {
    let mut findings = Vec::new();

    if !t.has_exif {
        findings.push(Finding {
            code: FlagCode::NoExif,
            title: "No EXIF metadata found".into(),
            explanation: "This file contains no EXIF metadata at all. A photo taken \
                by a camera or phone almost always carries EXIF data, so its absence on \
                a claimed original is suspicious — the metadata may have been stripped \
                by editing software, a re-save, or deliberate removal. (Note: messaging \
                apps and social media also strip EXIF, so absence alone is not proof of \
                tampering.)"
                .into(),
            detail: "No EXIF segment present in file".into(),
        });
        // Everything below needs EXIF data; nothing more to check.
        return findings;
    }

    if let Some(sw) = &t.software {
        findings.push(Finding {
            code: FlagCode::SoftwareTag,
            title: "Editing software tag present".into(),
            explanation: "The Software tag records the last program that wrote the \
                file. A value here (e.g. Photoshop, GIMP) means the image was saved by \
                software after capture — the file has been edited or at least re-encoded, \
                and is not a bit-for-bit camera original."
                .into(),
            detail: format!("Software tag: \"{sw}\""),
        });
    }

    match t.datetime_original_epoch() {
        None => findings.push(Finding {
            code: FlagCode::MissingDateTimeOriginal,
            title: "DateTimeOriginal missing or unparseable".into(),
            explanation: "Cameras stamp every shot with DateTimeOriginal — the moment \
                the shutter fired. Its absence (or a malformed value) on a claimed \
                original suggests the metadata was edited, stripped, or produced by \
                non-camera software."
                .into(),
            detail: match &t.datetime_original {
                Some(v) => format!("Unparseable DateTimeOriginal value: \"{v}\""),
                None => "DateTimeOriginal tag absent".into(),
            },
        }),
        Some(dto) => {
            if let Some(mtime) = t.file_mtime {
                if dto > mtime {
                    findings.push(Finding {
                        code: FlagCode::DateTimeMtimeMismatch,
                        title: "DateTimeOriginal is later than the file's modification time".into(),
                        explanation: "The EXIF timestamp says the photo was taken AFTER \
                            the file on disk was last modified — which is impossible for \
                            an untouched original. Either the capture timestamp was forged, \
                            or the file's mtime was tampered with (e.g. via touch). (Note: \
                            the reverse — mtime later than capture — is normal, since files \
                            get copied and re-saved.)"
                            .into(),
                        detail: format!(
                            "DateTimeOriginal: {}, file mtime: {}",
                            t.datetime_original.as_deref().unwrap_or("?"),
                            format_epoch(mtime)
                        ),
                    });
                }
            }
        }
    }

    if t.make.is_none() || t.model.is_none() {
        let missing: Vec<&str> = [
            t.make.is_none().then_some("Make"),
            t.model.is_none().then_some("Model"),
        ]
        .into_iter()
        .flatten()
        .collect();
        findings.push(Finding {
            code: FlagCode::MissingCameraInfo,
            title: "Camera Make/Model missing".into(),
            explanation: "Every digital camera and phone records its Make and Model. \
                Missing camera identification on a claimed original means the metadata \
                was stripped or the file never passed through a camera."
                .into(),
            detail: format!("Missing: {}", missing.join(", ")),
        });
    }

    if t.thumbnail_present {
        if let (Some((tw, th)), Some((iw, ih))) = (t.thumbnail_dimensions, t.image_dimensions) {
            let ratio_thumb = tw as f64 / th as f64;
            let ratio_main = iw as f64 / ih as f64;
            let rel_diff = ((ratio_thumb - ratio_main) / ratio_main).abs();
            if rel_diff > 0.01 {
                findings.push(Finding {
                    code: FlagCode::ThumbnailMismatch,
                    title: "Embedded thumbnail dimensions don't match the main image".into(),
                    explanation: "Cameras generate the embedded preview thumbnail at capture \
                        time, with the same aspect ratio as the photo. Many editors update \
                        the main image but leave the original thumbnail behind (or vice \
                        versa). A mismatch means the thumbnail and the visible image come \
                        from different moments in the file's history — a classic sign of \
                        editing."
                        .into(),
                    detail: format!(
                        "Thumbnail: {}x{} (ratio {:.3}), main image: {}x{} (ratio {:.3})",
                        tw, th, ratio_thumb, iw, ih, ratio_main
                    ),
                });
            }
        }
    }

    findings
}

/// Format epoch seconds as a readable UTC date-time (for detail strings).
fn format_epoch(epoch: i64) -> String {
    let days = epoch.div_euclid(86400);
    let secs = epoch.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clean_data() -> TagData {
        // A plausible, unflagged camera original.
        TagData {
            has_exif: true,
            software: None,
            datetime_original: Some("2024:03:10 14:22:01".into()),
            file_mtime: Some(crate::tags::parse_exif_datetime("2024:03:11 09:00:00").unwrap()),
            make: Some("Canon".into()),
            model: Some("Canon EOS R5".into()),
            image_dimensions: Some((8192, 5464)),
            thumbnail_present: true,
            thumbnail_dimensions: Some((160, 107)),
        }
    }

    fn codes(findings: &[Finding]) -> Vec<FlagCode> {
        findings.iter().map(|f| f.code).collect()
    }

    #[test]
    fn clean_original_has_no_flags() {
        assert!(detect_flags(&clean_data()).is_empty());
    }

    #[test]
    fn missing_exif_is_flagged_and_short_circuits() {
        let mut t = clean_data();
        t.has_exif = false;
        t.software = Some("GIMP 2.10".into()); // would flag if EXIF were parsed
        let findings = detect_flags(&t);
        assert_eq!(codes(&findings), vec![FlagCode::NoExif]);
        assert!(findings[0].explanation.contains("absence"));
    }

    #[test]
    fn software_tag_is_flagged_with_value() {
        let mut t = clean_data();
        t.software = Some("Adobe Photoshop 25.0 (Windows)".into());
        let findings = detect_flags(&t);
        assert_eq!(codes(&findings), vec![FlagCode::SoftwareTag]);
        assert!(findings[0].detail.contains("Photoshop"));
    }

    #[test]
    fn missing_datetime_original_is_flagged() {
        let mut t = clean_data();
        t.datetime_original = None;
        let findings = detect_flags(&t);
        assert_eq!(codes(&findings), vec![FlagCode::MissingDateTimeOriginal]);
    }

    #[test]
    fn unparseable_datetime_original_is_flagged() {
        let mut t = clean_data();
        t.datetime_original = Some("not a date".into());
        let findings = detect_flags(&t);
        assert_eq!(codes(&findings), vec![FlagCode::MissingDateTimeOriginal]);
        assert!(findings[0].detail.contains("not a date"));
    }

    #[test]
    fn datetime_after_mtime_is_impossible_and_flagged() {
        let mut t = clean_data();
        // Taken "2024-03-12" but file last written 2024-03-11.
        t.datetime_original = Some("2024:03:12 08:00:00".into());
        t.file_mtime = Some(crate::tags::parse_exif_datetime("2024:03:11 09:00:00").unwrap());
        let findings = detect_flags(&t);
        assert_eq!(codes(&findings), vec![FlagCode::DateTimeMtimeMismatch]);
    }

    #[test]
    fn datetime_before_mtime_is_normal_and_not_flagged() {
        let mut t = clean_data();
        // Taken 2024-03-10, copied/re-saved 2024-06-01 — perfectly normal.
        t.file_mtime = Some(crate::tags::parse_exif_datetime("2024:06:01 12:00:00").unwrap());
        assert!(detect_flags(&t).is_empty());
    }

    #[test]
    fn missing_make_and_model_are_flagged() {
        let mut t = clean_data();
        t.make = None;
        t.model = None;
        let findings = detect_flags(&t);
        assert_eq!(codes(&findings), vec![FlagCode::MissingCameraInfo]);
        assert_eq!(findings[0].detail, "Missing: Make, Model");
    }

    #[test]
    fn missing_model_only_names_model() {
        let mut t = clean_data();
        t.model = None;
        let findings = detect_flags(&t);
        assert_eq!(findings[0].detail, "Missing: Model");
    }

    #[test]
    fn thumbnail_aspect_mismatch_is_flagged() {
        let mut t = clean_data();
        t.thumbnail_dimensions = Some((160, 160)); // square thumb vs 3:2 image
        let findings = detect_flags(&t);
        assert_eq!(codes(&findings), vec![FlagCode::ThumbnailMismatch]);
    }

    #[test]
    fn thumbnail_with_matching_ratio_is_fine() {
        let mut t = clean_data();
        t.thumbnail_dimensions = Some((320, 213)); // 3:2-ish, within tolerance
        assert!(detect_flags(&t).is_empty());
    }

    #[test]
    fn thumbnail_mismatch_needs_known_dimensions() {
        let mut t = clean_data();
        t.thumbnail_dimensions = None; // present but dims unknown → cannot compare
        assert!(detect_flags(&t).is_empty());
    }

    #[test]
    fn no_thumbnail_no_flag() {
        let mut t = clean_data();
        t.thumbnail_present = false;
        assert!(detect_flags(&t).is_empty());
    }

    #[test]
    fn multiple_flags_stack_in_order() {
        let t = TagData {
            has_exif: true,
            software: Some("GIMP 2.10.38".into()),
            datetime_original: None,
            file_mtime: Some(1_700_000_000),
            make: None,
            model: Some("Pixel".into()),
            image_dimensions: Some((4000, 3000)),
            thumbnail_present: true,
            thumbnail_dimensions: Some((100, 100)),
        };
        let findings = detect_flags(&t);
        assert_eq!(
            codes(&findings),
            vec![
                FlagCode::SoftwareTag,
                FlagCode::MissingDateTimeOriginal,
                FlagCode::MissingCameraInfo,
                FlagCode::ThumbnailMismatch,
            ]
        );
        // Every finding must carry a human explanation.
        for f in &findings {
            assert!(!f.explanation.is_empty());
            assert!(!f.detail.is_empty());
        }
    }

    #[test]
    fn exif_datetime_parsing() {
        assert_eq!(
            crate::tags::parse_exif_datetime("1970:01:01 00:00:00"),
            Some(0)
        );
        assert_eq!(
            crate::tags::parse_exif_datetime("2024:03:10 14:22:01"),
            Some(1710080521)
        );
        assert_eq!(crate::tags::parse_exif_datetime("garbage"), None);
        assert_eq!(
            crate::tags::parse_exif_datetime("2024:13:01 00:00:00"),
            None
        );
    }
}
