//! What the info panel shows: file facts and a photo's EXIF.
use crate::media::{Entry, human_size};
use std::{io::BufReader, time::UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Meta {
    pub rows: Vec<(String, String)>,
    pub camera: Vec<(String, String)>,
}

/// `2026-11-22 14:03` in local time from Unix seconds.
pub fn format_time(utc_secs: i64) -> String {
    let t = utc_secs + i64::from(zgx::local_time::offset());
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    const MONTHS: [&str; 12] =
        ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!(
        "{} {d}, {y} · {:02}:{:02}",
        MONTHS[(m - 1) as usize],
        secs / 3600,
        (secs % 3600) / 60
    )
}

pub fn read(entry: &Entry, dims: Option<(u32, u32)>, duration: Option<f64>) -> Meta {
    let mut meta = Meta::default();
    let push = |rows: &mut Vec<(String, String)>, k: &str, v: String| {
        if !v.trim().is_empty() {
            rows.push((k.to_string(), v));
        }
    };
    if let Some((w, h)) = dims {
        let mp = w as f64 * h as f64 / 1e6;
        let extra = if mp >= 1. { format!("  ·  {mp:.1} MP") } else { String::new() };
        push(&mut meta.rows, "Dimensions", format!("{w} × {h}{extra}"));
    }
    if let Some(d) = duration.filter(|d| *d > 0.) {
        push(&mut meta.rows, "Duration", format_duration(d));
    }
    if let Ok(md) = std::fs::metadata(&entry.path) {
        push(&mut meta.rows, "Size", human_size(md.len()));
        let secs = |t: std::io::Result<std::time::SystemTime>| {
            t.ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| format_time(d.as_secs() as i64))
        };
        if let Some(m) = secs(md.modified()) {
            push(&mut meta.rows, "Modified", m);
        }
        if let Some(c) = secs(md.created()) {
            push(&mut meta.rows, "Created", c);
        }
    }
    let ext = entry
        .path
        .extension()
        .map(|e| e.to_string_lossy().to_uppercase())
        .unwrap_or_default();
    push(&mut meta.rows, "Type", ext);
    if let Some(dir) = entry.path.parent() {
        push(&mut meta.rows, "Folder", dir.display().to_string());
    }
    if let Ok(file) = std::fs::File::open(&entry.path) {
        if let Ok(exif) = exif::Reader::new().read_from_container(&mut BufReader::new(file)) {
            use exif::{In, Tag};
            let get = |tag| {
                exif.get_field(tag, In::PRIMARY)
                    .map(|f| f.display_value().with_unit(&exif).to_string().replace('"', ""))
            };
            let make = get(Tag::Make).unwrap_or_default();
            let model = get(Tag::Model).unwrap_or_default();
            let camera = if model.starts_with(make.as_str()) || make.is_empty() {
                model
            } else {
                format!("{make} {model}")
            };
            push(&mut meta.camera, "Camera", camera);
            push(&mut meta.camera, "Lens", get(Tag::LensModel).unwrap_or_default());
            push(&mut meta.camera, "Taken", get(Tag::DateTimeOriginal).unwrap_or_default());
            let exposure = [
                get(Tag::FNumber),
                get(Tag::ExposureTime),
                get(Tag::PhotographicSensitivity).map(|i| format!("ISO {i}")),
                get(Tag::FocalLength),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join("  ·  ");
            push(&mut meta.camera, "Exposure", exposure);
            if let (Some(lat), Some(lon)) = (get(Tag::GPSLatitude), get(Tag::GPSLongitude)) {
                let ns = get(Tag::GPSLatitudeRef).unwrap_or_default();
                let ew = get(Tag::GPSLongitudeRef).unwrap_or_default();
                push(&mut meta.camera, "Location", format!("{lat} {ns}, {lon} {ew}"));
            }
        }
    }
    meta
}

pub fn format_duration(secs: f64) -> String {
    let s = secs.max(0.) as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

#[cfg(test)]
mod tests {
    #[test]
    fn epoch_formats() {
        assert!(super::format_time(0).contains("1970") || super::format_time(0).contains("1969"));
        assert_eq!(super::format_duration(75.4), "1:15");
        assert_eq!(super::format_duration(3725.), "1:02:05");
    }
}
