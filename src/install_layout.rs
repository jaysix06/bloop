//! Where an installed Bloop lives. Shared by the app (for `--uninstall`)
//! and the installer (`installer/`, which includes this file by path).
#![allow(dead_code)]
use std::path::PathBuf;

/// Matches `Application::application_id` in the app, so the Start menu
/// shortcut and the running window group together on the taskbar.
pub const APP_ID: &str = "Bloop.Viewer";
pub const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\Bloop";
pub const EXE_NAME: &str = "bloop.exe";

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_default()
}

/// `%LOCALAPPDATA%\Programs\Bloop`: the per-user apps folder, no admin needed.
pub fn install_dir() -> PathBuf {
    env_dir("LOCALAPPDATA").join("Programs").join("Bloop")
}

pub fn installed_exe() -> PathBuf {
    install_dir().join(EXE_NAME)
}

pub fn start_menu_shortcut() -> PathBuf {
    env_dir("APPDATA").join(r"Microsoft\Windows\Start Menu\Programs\Bloop.lnk")
}

/// The user's desktop, wherever Windows put it (OneDrive moves it).
pub fn desktop_shortcut() -> PathBuf {
    use windows::Win32::UI::Shell::{FOLDERID_Desktop, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
    // SAFETY: the returned string is copied out before it is freed.
    let dir = unsafe {
        SHGetKnownFolderPath(&FOLDERID_Desktop, KF_FLAG_DEFAULT, None)
            .ok()
            .map(|p| {
                let s = p.to_string().unwrap_or_default();
                windows::Win32::System::Com::CoTaskMemFree(Some(p.0 as _));
                PathBuf::from(s)
            })
    };
    dir.unwrap_or_else(|| env_dir("USERPROFILE").join("Desktop")).join("Bloop.lnk")
}
