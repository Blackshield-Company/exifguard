use crate::flags::Finding;
use crate::tags::TagData;
use serde::Serialize;

/// Full analysis result for one file.
#[derive(Debug, Serialize)]
pub struct AnalysisReport {
    pub path: String,
    pub tags: TagData,
    pub findings: Vec<Finding>,
    /// True when no findings were raised.
    pub clean: bool,
}

impl AnalysisReport {
    pub fn new(path: String, tags: TagData, findings: Vec<Finding>) -> Self {
        let clean = findings.is_empty();
        AnalysisReport {
            path,
            tags,
            findings,
            clean,
        }
    }
}

/// Human-readable detailed report for `check`.
pub fn format_check_report(r: &AnalysisReport) -> String {
    let mut out = String::new();
    out.push_str(&format!("exifguard report: {}\n", r.path));
    out.push_str(&"=".repeat(60));
    out.push('\n');

    out.push_str("\nMetadata summary\n----------------\n");
    out.push_str(&format!("EXIF present:      {}\n", yes_no(r.tags.has_exif)));
    out.push_str(&format!(
        "Camera Make:       {}\n",
        r.tags.make.as_deref().unwrap_or("(missing)")
    ));
    out.push_str(&format!(
        "Camera Model:      {}\n",
        r.tags.model.as_deref().unwrap_or("(missing)")
    ));
    out.push_str(&format!(
        "DateTimeOriginal:  {}\n",
        r.tags.datetime_original.as_deref().unwrap_or("(missing)")
    ));
    out.push_str(&format!(
        "Software:          {}\n",
        r.tags.software.as_deref().unwrap_or("(none)")
    ));
    out.push_str(&format!(
        "Image dimensions:  {}\n",
        fmt_dims(r.tags.image_dimensions)
    ));
    out.push_str(&format!(
        "Thumbnail:         {}\n",
        if r.tags.thumbnail_present {
            format!("present ({})", fmt_dims(r.tags.thumbnail_dimensions))
        } else {
            "absent".to_string()
        }
    ));

    if r.clean {
        out.push_str("\nFindings\n--------\nNo flags raised. ");
        out.push_str("This does NOT prove the photo is authentic — only that none of the\n");
        out.push_str("metadata checks in exifguard found anomalies.\n");
    } else {
        out.push_str(&format!(
            "\nFindings ({} flag{} raised)\n--------\n",
            r.findings.len(),
            if r.findings.len() == 1 { "" } else { "s" }
        ));
        for (i, f) in r.findings.iter().enumerate() {
            out.push_str(&format!("\n{}. [{}] {}\n", i + 1, f.code, f.title));
            out.push_str(&format!("   Detail: {}\n", f.detail));
            out.push_str(&format!("   Why it matters: {}\n", f.explanation));
        }
        out.push_str(
            "\nReminder: flags are investigative leads, not proof of tampering. See the\n",
        );
        out.push_str("README's \"Honest limits\" section.\n");
    }
    out
}

/// Human-readable one-line-per-file table for `scan`.
pub fn format_scan_table(reports: &[AnalysisReport]) -> String {
    let mut out = String::new();
    if reports.is_empty() {
        out.push_str("No JPEG/JPG files found.\n");
        return out;
    }
    let path_w = reports
        .iter()
        .map(|r| r.path.len())
        .max()
        .unwrap_or(4)
        .max(4);
    out.push_str(&format!(
        "{:<path_w$}  {:>5}  {}\n",
        "PATH",
        "FLAGS",
        "CODES",
        path_w = path_w
    ));
    out.push_str(&format!(
        "{:-<path_w$}  {:-<5}  {:-<20}\n",
        "",
        "",
        "",
        path_w = path_w
    ));
    for r in reports {
        let codes = r
            .findings
            .iter()
            .map(|f| f.code.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "{:<path_w$}  {:>5}  {}\n",
            r.path,
            r.findings.len(),
            if codes.is_empty() {
                "clean".to_string()
            } else {
                codes
            },
            path_w = path_w
        ));
    }
    let flagged = reports.iter().filter(|r| !r.clean).count();
    out.push_str(&format!(
        "\n{} file(s) scanned, {} flagged.\n",
        reports.len(),
        flagged
    ));
    out.push_str("Flags are leads, not proof — run `exifguard check <file>` for details.\n");
    out
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

fn fmt_dims(d: Option<(u32, u32)>) -> String {
    match d {
        Some((w, h)) => format!("{w}x{h}"),
        None => "(unknown)".into(),
    }
}
