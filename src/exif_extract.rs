use crate::flags::detect_flags;
use crate::report::AnalysisReport;
use crate::tags::TagData;
use anyhow::{Context, Result};
use exif::{Exif, In, Reader, Tag, Value};
use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Analyze one image file: extract EXIF tag data and run flag detection.
pub fn analyze_file(path: &Path) -> Result<AnalysisReport> {
    let bytes = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;

    let mut tags = TagData {
        file_mtime: fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64),
        image_dimensions: jpeg_dimensions(&bytes),
        ..Default::default()
    };

    let file = fs::File::open(path)?;
    if let Ok(exif) = Reader::new().read_from_container(&mut BufReader::new(file)) {
        tags.has_exif = true;
        tags.software = ascii_field(&exif, Tag::Software, In::PRIMARY);
        tags.make = ascii_field(&exif, Tag::Make, In::PRIMARY);
        tags.model = ascii_field(&exif, Tag::Model, In::PRIMARY);
        // DateTimeOriginal lives in the EXIF sub-IFD (ifd_num 0 in
        // kamadak-exif's numbering for sub-IFDs is In::PRIMARY? No — the
        // EXIF sub-IFD fields carry ifd_num In(0) with the tag being
        // DateTimeOriginal; kamadak-exif flattens sub-IFDs into the field
        // list. Search all IFDs for the tag.
        tags.datetime_original = ascii_field_any(&exif, Tag::DateTimeOriginal);

        // EXIF-declared pixel dimensions, if present (fallback for SOF parse).
        if tags.image_dimensions.is_none() {
            let w = uint_field_any(&exif, Tag::PixelXDimension);
            let h = uint_field_any(&exif, Tag::PixelYDimension);
            if let (Some(w), Some(h)) = (w, h) {
                tags.image_dimensions = Some((w, h));
            }
        }

        // Thumbnail = IFD1. kamadak-exif exposes it as ifd_num In(1).
        let thumb_fields: Vec<_> = exif.fields().filter(|f| f.ifd_num == In(1)).collect();
        tags.thumbnail_present = !thumb_fields.is_empty();
        let tw = uint_field(&exif, Tag::PixelXDimension, In(1));
        let th = uint_field(&exif, Tag::PixelYDimension, In(1));
        if let (Some(w), Some(h)) = (tw, th) {
            tags.thumbnail_dimensions = Some((w, h));
        }
    }

    let findings = detect_flags(&tags);
    Ok(AnalysisReport::new(
        path.display().to_string(),
        tags,
        findings,
    ))
}

fn ascii_field(exif: &Exif, tag: Tag, ifd_num: In) -> Option<String> {
    exif.get_field(tag, ifd_num).and_then(|f| match &f.value {
        Value::Ascii(v) => v
            .first()
            .map(|b| {
                String::from_utf8_lossy(b)
                    .trim_end_matches('\0')
                    .trim()
                    .to_string()
            })
            .filter(|s| !s.is_empty()),
        _ => None,
    })
}

fn ascii_field_any(exif: &Exif, tag: Tag) -> Option<String> {
    exif.fields()
        .find(|f| f.tag == tag)
        .and_then(|f| match &f.value {
            Value::Ascii(v) => v
                .first()
                .map(|b| {
                    String::from_utf8_lossy(b)
                        .trim_end_matches('\0')
                        .trim()
                        .to_string()
                })
                .filter(|s| !s.is_empty()),
            _ => f.display_value().to_string().into(),
        })
}

fn uint_field(exif: &Exif, tag: Tag, ifd_num: In) -> Option<u32> {
    exif.get_field(tag, ifd_num).and_then(|f| match &f.value {
        Value::Short(v) => v.first().map(|&x| x as u32),
        Value::Long(v) => v.first().copied(),
        _ => None,
    })
}

fn uint_field_any(exif: &Exif, tag: Tag) -> Option<u32> {
    exif.fields()
        .find(|f| f.tag == tag)
        .and_then(|f| match &f.value {
            Value::Short(v) => v.first().map(|&x| x as u32),
            Value::Long(v) => v.first().copied(),
            _ => None,
        })
}

/// Parse width/height directly from the JPEG SOF marker — no EXIF needed.
/// Scans marker segments until a Start Of Frame marker is found.
pub fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 4 || bytes[0] != 0xFF || bytes[1] != 0xD8 {
        return None;
    }
    let mut i = 2;
    while i + 3 < bytes.len() {
        if bytes[i] != 0xFF {
            i += 1;
            continue;
        }
        let marker = bytes[i + 1];
        // SOF markers (excluding DHT/DAC/arith variants).
        if matches!(marker, 0xC0..=0xCF)
            && !matches!(marker, 0xC4 | 0xC8 | 0xCC)
            && i + 8 < bytes.len()
        {
            let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            if w > 0 && h > 0 {
                return Some((w, h));
            }
            return None;
        }
        // Standalone markers without length.
        if marker == 0xD8 || marker == 0xD9 || (0xD0..=0xD7).contains(&marker) {
            i += 2;
            continue;
        }
        if i + 3 >= bytes.len() {
            break;
        }
        let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        if len < 2 {
            break;
        }
        i += 2 + len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_dims_from_minimal_sof() {
        // SOI + SOF0 (height 1080, width 1920) + EOI
        let jpg = [
            0xFF, 0xD8, // SOI
            0xFF, 0xC0, 0x00, 0x11, // SOF0, len 17
            0x08, // precision
            0x04, 0x38, // height 1080
            0x07, 0x80, // width 1920
            0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01, // components
            0xFF, 0xD9, // EOI
        ];
        assert_eq!(jpeg_dimensions(&jpg), Some((1920, 1080)));
    }

    #[test]
    fn jpeg_dims_rejects_non_jpeg() {
        assert_eq!(jpeg_dimensions(b"hello world"), None);
        assert_eq!(jpeg_dimensions(&[]), None);
    }
}
