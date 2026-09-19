use serde::Serialize;

/// Normalized tag data extracted from one image file.
///
/// This is the input to the flag-detection logic. It is deliberately
/// plain data so unit tests can construct cases directly without
/// needing real camera files.
#[derive(Debug, Clone, Default, Serialize)]
pub struct TagData {
    /// Whether any EXIF data was found at all.
    pub has_exif: bool,
    /// Software tag (EXIF tag 0x0131), e.g. "Adobe Photoshop".
    pub software: Option<String>,
    /// DateTimeOriginal (EXIF tag 0x9003), raw string "YYYY:MM:DD HH:MM:SS".
    pub datetime_original: Option<String>,
    /// File modification time as Unix epoch seconds.
    pub file_mtime: Option<i64>,
    /// Camera Make (EXIF tag 0x010F).
    pub make: Option<String>,
    /// Camera Model (EXIF tag 0x0110).
    pub model: Option<String>,
    /// Main image dimensions in pixels (width, height), from EXIF or the
    /// JPEG SOF marker.
    pub image_dimensions: Option<(u32, u32)>,
    /// Whether an embedded thumbnail (EXIF IFD1) is present.
    pub thumbnail_present: bool,
    /// Embedded thumbnail dimensions in pixels (width, height), if known.
    pub thumbnail_dimensions: Option<(u32, u32)>,
}

impl TagData {
    /// Parse the EXIF DateTimeOriginal string into Unix epoch seconds
    /// (interpreted as UTC, since EXIF carries no timezone unless
    /// OffsetTimeOriginal is set).
    pub fn datetime_original_epoch(&self) -> Option<i64> {
        let s = self.datetime_original.as_deref()?.trim().to_string();
        parse_exif_datetime(&s)
    }
}

/// Parse "YYYY:MM:DD HH:MM:SS" into epoch seconds (UTC assumption).
/// Returns None for malformed input. No chrono dependency — uses the
/// civil-from-days algorithm in reverse.
pub fn parse_exif_datetime(s: &str) -> Option<i64> {
    let mut parts = s.split_whitespace();
    let date = parts.next()?;
    let time = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let d: Vec<i64> = date
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let t: Vec<i64> = time
        .split(':')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    if d.len() != 3 || t.len() != 3 {
        return None;
    }
    let (y, m, day) = (d[0], d[1], d[2]);
    let (hh, mm, ss) = (t[0], t[1], t[2]);
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || hh > 23 || mm > 59 || ss > 60 {
        return None;
    }
    // Days from civil (Howard Hinnant's algorithm).
    let y_adj = if m <= 2 { y - 1 } else { y };
    let era = if y_adj >= 0 { y_adj } else { y_adj - 399 } / 400;
    let yoe = y_adj - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + hh * 3600 + mm * 60 + ss)
}
