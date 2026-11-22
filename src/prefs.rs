//! Small remembered preferences in %APPDATA%\Bloop\prefs.json.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Default, Serialize, Deserialize)]
pub struct Prefs {
    /// The "Make Bloop as Default" dialog has been offered on first run.
    #[serde(default)]
    pub default_prompted: bool,
}

fn file() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("Bloop");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("prefs.json"))
}

pub fn get() -> Prefs {
    file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn update(change: impl FnOnce(&mut Prefs)) {
    let mut p = get();
    change(&mut p);
    if let (Some(f), Ok(s)) = (file(), serde_json::to_string_pretty(&p)) {
        let _ = std::fs::write(f, s);
    }
}
