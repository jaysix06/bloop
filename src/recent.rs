//! Recently opened folders, for the start screen. A plain text file in
//! %APPDATA%\Bloop, one path per line.
use std::path::{Path, PathBuf};

const MAX: usize = 8;

fn file() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("Bloop");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("recent.txt"))
}

pub fn load() -> Vec<PathBuf> {
    file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .map(|s| {
            let mut out: Vec<PathBuf> = Vec::new();
            for p in s.lines().map(|l| crate::media::normalize(Path::new(l))).filter(|p| p.is_dir()) {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
            out.truncate(MAX);
            out
        })
        .unwrap_or_default()
}

pub fn push(dir: &Path) -> Vec<PathBuf> {
    let mut list = load();
    list.retain(|p| p != dir);
    list.insert(0, dir.to_path_buf());
    list.truncate(MAX);
    if let Some(f) = file() {
        let text: Vec<String> = list.iter().map(|p| p.display().to_string()).collect();
        let _ = std::fs::write(f, text.join("\n"));
    }
    list
}
