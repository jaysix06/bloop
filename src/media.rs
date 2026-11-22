//! What Bloop can open, and listing a folder's siblings in Explorer order.
use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MediaKind {
    Image,
    Gif,
    Svg,
    Video,
}

const IMAGE: &[&str] = &[
    "jpg", "jpeg", "jpe", "jfif", "png", "apng", "bmp", "dib", "webp", "tif", "tiff", "ico", "tga",
    "avif", "heic", "heif", "jxr", "wdp",
];
const VIDEO: &[&str] = &[
    "mp4", "m4v", "mov", "webm", "mkv", "avi", "wmv", "3gp", "mpg", "mpeg", "ts", "m2ts",
];

pub fn kind_of(path: &Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    let ext = ext.as_str();
    if ext == "gif" {
        Some(MediaKind::Gif)
    } else if ext == "svg" {
        Some(MediaKind::Svg)
    } else if IMAGE.contains(&ext) {
        Some(MediaKind::Image)
    } else if VIDEO.contains(&ext) {
        Some(MediaKind::Video)
    } else {
        None
    }
}

/// Every extension Bloop registers for "Open with".
pub fn all_extensions() -> impl Iterator<Item = (&'static str, MediaKind)> {
    IMAGE
        .iter()
        .map(|e| (*e, MediaKind::Image))
        .chain([("gif", MediaKind::Gif), ("svg", MediaKind::Svg)])
        .chain(VIDEO.iter().map(|e| (*e, MediaKind::Video)))
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub path: PathBuf,
    pub kind: MediaKind,
}

impl Entry {
    pub fn name(&self) -> String {
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Explorer's ordering: "img2" before "img10", case-insensitive.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        let (a, b) = (HSTRING::from(a), HSTRING::from(b));
        // SAFETY: both are valid NUL-terminated wide strings.
        let r = unsafe { windows::Win32::UI::Shell::StrCmpLogicalW(&a, &b) };
        r.cmp(&0)
    }
    #[cfg(not(windows))]
    a.to_lowercase().cmp(&b.to_lowercase())
}

/// The media files in `dir`, sorted as Explorer sorts them by name.
pub fn scan_dir(dir: &Path) -> Vec<Entry> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut entries: Vec<(String, Entry)> = read
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .filter_map(|e| {
            let path = e.path();
            let kind = kind_of(&path)?;
            let name = e.file_name().to_string_lossy().into_owned();
            Some((name, Entry { path, kind }))
        })
        .collect();
    entries.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    entries.into_iter().map(|(_, e)| e).collect()
}

/// Opening a file gives its folder and its position in it; opening a
/// folder gives the folder from its first item.
pub fn open_target(path: &Path) -> (Vec<Entry>, usize) {
    let path = &normalize(path);
    if path.is_dir() {
        return (scan_dir(path), 0);
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut entries = scan_dir(dir);
    let index = entries.iter().position(|e| same_file(&e.path, path));
    match index {
        Some(i) => (entries, i),
        None => {
            // Opened something whose extension we do not list: show it alone.
            entries.clear();
            entries.push(Entry {
                path: path.to_path_buf(),
                kind: kind_of(path).unwrap_or(MediaKind::Image),
            });
            (entries, 0)
        }
    }
}

/// A clean absolute Windows path: backslashes, long names and no verbatim
/// prefix, since Explorer's APIs reject the other spellings.
pub fn normalize(path: &Path) -> PathBuf {
    let p = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let s = p.to_string_lossy().replace('/', "\\");
    let s = if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else {
        s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()
    };
    PathBuf::from(s)
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
        || matches!((a.canonicalize(), b.canonicalize()), (Ok(x), Ok(y)) if x == y)
}

/// `file:///C:/a%20b.jpg` from a second launch, back to a path.
pub fn path_from_url(url: &str) -> Option<PathBuf> {
    let rest = url.strip_prefix("file:///").or_else(|| url.strip_prefix("file://"))?;
    let mut bytes = Vec::with_capacity(rest.len());
    let raw = rest.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(v) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                bytes.push(v);
                i += 3;
                continue;
            }
        }
        bytes.push(raw[i]);
        i += 1;
    }
    let s = String::from_utf8(bytes).ok()?;
    Some(PathBuf::from(s.replace('/', "\\")))
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024. && u < UNITS.len() - 1 {
        v /= 1024.;
        u += 1;
    }
    if u == 0 { format!("{bytes} B") } else { format!("{v:.1} {}", UNITS[u]) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn urls_become_paths() {
        assert_eq!(
            path_from_url("file:///C:/a%20b/c.jpg").unwrap(),
            PathBuf::from("C:\\a b\\c.jpg")
        );
    }
    #[test]
    fn natural_order() {
        assert_eq!(natural_cmp("img2.jpg", "img10.jpg"), Ordering::Less);
        assert_eq!(natural_cmp("B.jpg", "a.jpg"), Ordering::Greater);
    }
    #[test]
    fn kinds() {
        assert_eq!(kind_of(Path::new("x.JPG")), Some(MediaKind::Image));
        assert_eq!(kind_of(Path::new("x.gif")), Some(MediaKind::Gif));
        assert_eq!(kind_of(Path::new("x.mp4")), Some(MediaKind::Video));
        assert_eq!(kind_of(Path::new("x.txt")), None);
    }
}
