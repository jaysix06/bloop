//! Automatic updates from GitHub Releases.
//!
//! Bloop asks `api.github.com/repos/<owner>/<repo>/releases/latest` (the repo
//! is `repository` in Cargo.toml) at most once a day. A newer tag with a
//! `Bloop-Setup-<version>.exe` asset is downloaded in the background,
//! checked against its `.sha256` asset, and then installed by running that
//! setup silently, now (restart) or when Bloop closes.
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const CURRENT: &str = env!("CARGO_PKG_VERSION");
const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const DAY: u64 = 24 * 60 * 60;
/// No sane installer is this big; refuse anything larger.
const MAX_DOWNLOAD: u64 = 300 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub page: String,
    asset_url: String,
    asset_name: String,
    asset_size: u64,
    sha_url: Option<String>,
}

/// What the background worker reports to the UI.
pub enum Event {
    UpToDate,
    Found(Release),
    Progress(f32),
    Ready(Release, PathBuf),
    Failed(String),
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// `owner/repo` from `https://github.com/owner/repo`, if it is set up.
fn repo_slug() -> Option<&'static str> {
    let slug = REPOSITORY.strip_prefix("https://github.com/")?.trim_end_matches('/');
    (slug.contains('/') && !slug.starts_with("OWNER/")).then_some(slug)
}

pub fn is_configured() -> bool {
    repo_slug().is_some()
}

/// `1.2.3` from `v1.2.3`; anything unparsable compares as 0.0.0.
pub fn parse_version(v: &str) -> (u64, u64, u64) {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next().unwrap_or("");
    let mut it = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    (it.next().unwrap_or(0), it.next().unwrap_or(0), it.next().unwrap_or(0))
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    parse_version(candidate) > parse_version(current)
}

// ----- remembered state ------------------------------------------------------

#[derive(Default, serde::Serialize, Deserialize)]
struct State {
    #[serde(default)]
    last_check: u64,
    #[serde(default)]
    skipped: Option<String>,
}

fn state_file() -> Option<PathBuf> {
    let dir = PathBuf::from(std::env::var_os("APPDATA")?).join("Bloop");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join("update.json"))
}

fn load_state() -> State {
    state_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_state(state: &State) {
    if let (Some(f), Ok(s)) = (state_file(), serde_json::to_string_pretty(state)) {
        let _ = std::fs::write(f, s);
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Whether the daily automatic check is due (always true for a manual one).
pub fn due() -> bool {
    now().saturating_sub(load_state().last_check) >= DAY
}

pub fn skip(version: &str) {
    let mut s = load_state();
    s.skipped = Some(version.to_string());
    save_state(&s);
}

/// True when running from the installed location: only an installed Bloop
/// updates itself (a dev build from `target\` never does on its own).
pub fn is_installed_copy() -> bool {
    let dir = crate::install_layout::install_dir();
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
        .is_some_and(|p| crate::media::normalize(&p) == crate::media::normalize(&dir))
}

// ----- network ---------------------------------------------------------------

fn agent() -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| e.to_string())?;
    Ok(ureq::AgentBuilder::new()
        .tls_connector(std::sync::Arc::new(tls))
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .user_agent(&format!("Bloop/{CURRENT} (update check)"))
        .build())
}

fn latest(agent: &ureq::Agent) -> Result<Option<Release>, String> {
    let slug = repo_slug().ok_or("No update source is set up for this build.")?;
    let url = format!("https://api.github.com/repos/{slug}/releases/latest");
    let resp = agent
        .get(&url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call();
    let release: GhRelease = match resp {
        Ok(r) => r.into_json().map_err(|e| format!("Unexpected reply from GitHub: {e}"))?,
        // No releases published yet.
        Err(ureq::Error::Status(404, _)) => return Ok(None),
        Err(ureq::Error::Status(code, _)) => return Err(format!("GitHub answered {code}.")),
        Err(e) => return Err(format!("Couldn't reach GitHub ({}).", short_err(&e))),
    };
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let version = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    let Some(asset) = release.assets.iter().find(|a| {
        let n = a.name.to_ascii_lowercase();
        n.starts_with("bloop-setup") && n.ends_with(".exe")
    }) else {
        return Ok(None);
    };
    let sha_url = release
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(&format!("{}.sha256", asset.name)))
        .map(|a| a.browser_download_url.clone());
    Ok(Some(Release {
        version,
        notes: release.body.unwrap_or_default(),
        page: release.html_url,
        asset_url: asset.browser_download_url.clone(),
        asset_name: asset.name.clone(),
        asset_size: asset.size,
        sha_url,
    }))
}

fn short_err(e: &ureq::Error) -> String {
    let s = e.to_string();
    s.split(':').next().unwrap_or(&s).to_string()
}

/// Check, and when there is something newer, download and verify it.
/// `manual` ignores the daily limit and a skipped version.
pub fn check_and_download(manual: bool, report: impl Fn(Event)) {
    let result = (|| -> Result<(), String> {
        let agent = agent()?;
        let mut state = load_state();
        state.last_check = now();
        save_state(&state);
        let Some(release) = latest(&agent)? else {
            report(Event::UpToDate);
            return Ok(());
        };
        let skipped = state.skipped.as_deref() == Some(release.version.as_str());
        if !is_newer(&release.version, CURRENT) || (skipped && !manual) {
            report(Event::UpToDate);
            return Ok(());
        }
        report(Event::Found(release.clone()));
        let path = download(&agent, &release, &report)?;
        report(Event::Ready(release, path));
        Ok(())
    })();
    if let Err(e) = result {
        report(Event::Failed(e));
    }
}

fn download(agent: &ureq::Agent, release: &Release, report: &impl Fn(Event)) -> Result<PathBuf, String> {
    if release.asset_size > MAX_DOWNLOAD {
        return Err("The update is unexpectedly large; skipping it.".into());
    }
    let dir = std::env::temp_dir().join("Bloop-Update");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Only a plain file name from the release, never a path.
    let name = Path::new(&release.asset_name)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .filter(|n| n.to_ascii_lowercase().ends_with(".exe"))
        .ok_or("Bad installer name in the release.")?;
    let target = dir.join(&name);

    let expected_sha = match &release.sha_url {
        Some(url) => {
            let text = agent
                .get(url)
                .call()
                .map_err(|e| format!("Couldn't fetch the checksum ({}).", short_err(&e)))?
                .into_string()
                .map_err(|e| e.to_string())?;
            let hex = text.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
            if hex.len() != 64 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err("The release's checksum file is malformed.".into());
            }
            Some(hex)
        }
        None => None,
    };

    // Already downloaded earlier and still intact?
    if let (Some(sha), Ok(bytes)) = (&expected_sha, std::fs::read(&target))
        && hex(&Sha256::digest(&bytes)) == *sha
    {
        report(Event::Progress(1.));
        return Ok(target);
    }

    let resp = agent
        .get(&release.asset_url)
        .call()
        .map_err(|e| format!("Download failed ({}).", short_err(&e)))?;
    let total = release.asset_size.max(1);
    let mut reader = resp.into_reader().take(MAX_DOWNLOAD);
    let mut hasher = Sha256::new();
    let partial = dir.join(format!("{name}.part"));
    let mut file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 256 * 1024];
    let mut got: u64 = 0;
    let mut last_report = 0.;
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Download interrupted ({e})."))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(|e| e.to_string())?;
        hasher.update(&buf[..n]);
        got += n as u64;
        let f = (got as f32 / total as f32).min(1.);
        if f - last_report >= 0.02 {
            last_report = f;
            report(Event::Progress(f));
        }
    }
    drop(file);
    if got != release.asset_size {
        let _ = std::fs::remove_file(&partial);
        return Err("The download was incomplete.".into());
    }
    if let Some(sha) = expected_sha
        && hex(&hasher.finalize()) != sha
    {
        let _ = std::fs::remove_file(&partial);
        return Err("The download didn't match its checksum, so it was discarded.".into());
    }
    let _ = std::fs::remove_file(&target);
    std::fs::rename(&partial, &target).map_err(|e| e.to_string())?;
    report(Event::Progress(1.));
    Ok(target)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Run the downloaded setup silently. With `relaunch`, it waits for this
/// process to exit and then starts the new Bloop.
pub fn launch_installer(setup: &Path, relaunch: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    let mut cmd = std::process::Command::new(setup);
    // Always wait for this process to exit first, so the old exe is free to
    // delete and nothing is left behind.
    cmd.args(["/S", "--wait-pid", &std::process::id().to_string()]);
    if relaunch {
        cmd.arg("--relaunch");
    }
    cmd.creation_flags(DETACHED_PROCESS).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn versions() {
        assert!(is_newer("v0.2.0", "0.1.9"));
        assert!(is_newer("0.1.10", "0.1.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1.0-beta", "0.1.0"));
        assert_eq!(parse_version("V1.2"), (1, 2, 0));
    }
}
