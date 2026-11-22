//! The installation itself: copy the embedded app, add shortcuts, the
//! Settings › Apps entry, and (optionally) "Open with" registration.
//! Everything is per-user under HKCU and %LOCALAPPDATA%, so no admin.
#[path = "../../src/install_layout.rs"]
pub mod layout;

use std::{
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::{
    Win32::{
        Storage::EnhancedStorage::PKEY_AppUserModel_ID,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            IPersistFile,
        },
        UI::Shell::{IShellLinkW, PropertiesSystem::IPropertyStore, ShellLink},
    },
    core::{HSTRING, Interface},
};

/// The release `bloop.exe`, embedded at build time.
static PAYLOAD: &[u8] = include_bytes!(env!("BLOOP_PAYLOAD"));
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Clone, Copy)]
pub struct Options {
    pub desktop: bool,
    pub register: bool,
}

pub enum Progress {
    Step(f32, &'static str),
    Done(PathBuf),
    Failed(String),
}

pub fn is_installed() -> bool {
    layout::installed_exe().exists()
}

/// For a silent install or update: keep what's there now (a desktop
/// shortcut, "Open with"), with "Open with" on for a fresh install.
pub fn silent_options() -> Options {
    let registered = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .open_subkey(r"Software\RegisteredApplications")
        .and_then(|k| k.get_value::<String, _>("Bloop"))
        .is_ok();
    Options {
        desktop: layout::desktop_shortcut().exists(),
        register: registered || !is_installed(),
    }
}

/// Wait (up to 15 s) for process `pid` to exit.
pub fn wait_for_exit(pid: u32) {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    // SAFETY: the handle is closed after waiting on it.
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let _ = WaitForSingleObject(h, 15_000);
            let _ = CloseHandle(h);
        }
    }
}

pub fn run(opts: Options, mut report: impl FnMut(Progress)) {
    match install(opts, &mut report) {
        Ok(exe) => report(Progress::Done(exe)),
        Err(e) => report(Progress::Failed(e)),
    }
}

fn install(opts: Options, report: &mut impl FnMut(Progress)) -> Result<PathBuf, String> {
    // SAFETY: first COM use on this worker thread; shell links want an STA.
    let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    let dir = layout::install_dir();
    let exe = layout::installed_exe();

    report(Progress::Step(0.1, "Preparing…"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create {}: {e}", dir.display()))?;
    clean_old_copies(&dir);

    report(Progress::Step(0.3, "Copying Bloop…"));
    if exe.exists() {
        // A running Bloop can't be overwritten, but it can be renamed away;
        // the old copy is cleaned up on the next install.
        let stale = dir.join(format!("bloop.exe.old-{}", std::process::id()));
        let _ = std::fs::rename(&exe, &stale);
    }
    std::fs::write(&exe, PAYLOAD).map_err(|e| format!("Couldn't write {}: {e}", exe.display()))?;

    report(Progress::Step(0.55, "Adding to the Start menu…"));
    shortcut(&layout::start_menu_shortcut(), &exe, &dir)
        .map_err(|e| format!("Couldn't create the Start menu shortcut: {}", e.message()))?;
    let desktop = layout::desktop_shortcut();
    if opts.desktop {
        let _ = shortcut(&desktop, &exe, &dir);
    } else {
        let _ = std::fs::remove_file(&desktop);
    }

    report(Progress::Step(0.75, "Adding to Settings › Apps…"));
    uninstall_entry(&exe, &dir).map_err(|e| format!("Couldn't register the uninstaller: {e}"))?;

    if opts.register {
        report(Progress::Step(0.9, "Adding to \"Open with\"…"));
        let status = std::process::Command::new(&exe)
            .args(["--register", "--quiet"])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        if !status.is_ok_and(|s| s.success()) {
            return Err("Bloop was installed, but adding it to \"Open with\" failed.".into());
        }
    }
    report(Progress::Step(1.0, "Done"));
    Ok(exe)
}

fn clean_old_copies(dir: &Path) {
    if let Ok(read) = std::fs::read_dir(dir) {
        for e in read.flatten() {
            if e.file_name().to_string_lossy().starts_with("bloop.exe.old-") {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

fn shortcut(lnk: &Path, target: &Path, workdir: &Path) -> windows::core::Result<()> {
    if let Some(parent) = lnk.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // SAFETY: COM calls on interfaces this function owns.
    unsafe {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(&HSTRING::from(target.as_os_str()))?;
        link.SetWorkingDirectory(&HSTRING::from(workdir.as_os_str()))?;
        link.SetIconLocation(&HSTRING::from(target.as_os_str()), 0)?;
        link.SetDescription(&HSTRING::from("Fast, quiet photo, GIF and video viewer"))?;
        // Same AppUserModelID as the running app, so a pinned shortcut and
        // the open window share one taskbar button.
        let store: IPropertyStore = link.cast()?;
        store.SetValue(&PKEY_AppUserModel_ID, &PROPVARIANT::from(layout::APP_ID))?;
        store.Commit()?;
        let file: IPersistFile = link.cast()?;
        file.Save(&HSTRING::from(lnk.as_os_str()), true)?;
    }
    Ok(())
}

fn uninstall_entry(exe: &Path, dir: &Path) -> std::io::Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(layout::UNINSTALL_KEY)?;
    let exe_s = exe.display().to_string();
    key.set_value("DisplayName", &"Bloop")?;
    key.set_value("DisplayVersion", &VERSION)?;
    key.set_value("Publisher", &"Bloop")?;
    key.set_value("DisplayIcon", &format!("\"{exe_s}\",0"))?;
    key.set_value("InstallLocation", &dir.display().to_string())?;
    key.set_value("UninstallString", &format!("\"{exe_s}\" --uninstall"))?;
    key.set_value("QuietUninstallString", &format!("\"{exe_s}\" --uninstall"))?;
    key.set_value("NoModify", &1u32)?;
    key.set_value("NoRepair", &1u32)?;
    key.set_value("EstimatedSize", &((PAYLOAD.len() / 1024) as u32))?;
    Ok(())
}
