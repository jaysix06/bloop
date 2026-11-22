//! Explorer integration: thumbnails, the Recycle Bin, "show in folder" and
//! per-user file associations. Nothing here shows Windows UI.
use crate::media::{MediaKind, all_extensions};
use std::{os::windows::process::CommandExt, path::Path, sync::Arc};
use windows::{
    Win32::{
        Foundation::{HWND, SIZE},
        Graphics::Gdi::*,
        UI::Shell::*,
    },
    core::{HSTRING, PCWSTR},
};
use zgx::image::ImageData;

/// Explorer's cached thumbnail (works for videos too), at most `side` square.
pub fn thumbnail(path: &Path, side: i32) -> Option<Arc<ImageData>> {
    crate::wic::com_init();
    // SAFETY: COM/GDI calls with owned handles that are released below.
    unsafe {
        let factory: IShellItemImageFactory =
            SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None).ok()?;
        let hbm = factory
            .GetImage(SIZE { cx: side, cy: side }, SIIGBF_RESIZETOFIT)
            .ok()?;
        let mut bm = BITMAP::default();
        GetObjectW(hbm.into(), size_of::<BITMAP>() as i32, Some(&mut bm as *mut _ as *mut _));
        let (w, h) = (bm.bmWidth, bm.bmHeight.abs());
        if w <= 0 || h <= 0 {
            let _ = DeleteObject(hbm.into());
            return None;
        }
        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let dc = CreateCompatibleDC(None);
        let lines = GetDIBits(
            dc,
            hbm,
            0,
            h as u32,
            Some(buf.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );
        let _ = DeleteDC(dc);
        let _ = DeleteObject(hbm.into());
        if lines == 0 {
            return None;
        }
        let opaque = buf.chunks_exact(4).all(|p| p[3] == 0);
        for p in buf.chunks_exact_mut(4) {
            p.swap(0, 2);
            if opaque {
                p[3] = 255;
            } else if p[3] > 0 && p[3] < 255 {
                // GDI hands back premultiplied alpha.
                let a = p[3] as f32 / 255.;
                for c in &mut p[..3] {
                    *c = ((*c as f32 / a).min(255.)) as u8;
                }
            }
        }
        ImageData::new(w as u32, h as u32, buf).ok().map(Arc::new)
    }
}

/// Send a file to the Recycle Bin without any shell dialog.
pub fn recycle(path: &Path) -> Result<(), String> {
    let mut from: Vec<u16> = path.as_os_str().encode_wide_vec();
    from.extend([0, 0]);
    let mut op = SHFILEOPSTRUCTW {
        hwnd: HWND::default(),
        wFunc: FO_DELETE,
        pFrom: PCWSTR(from.as_ptr()),
        fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI).0 as u16,
        ..Default::default()
    };
    // SAFETY: `from` is double-NUL terminated and outlives the call.
    let r = unsafe { SHFileOperationW(&mut op) };
    if r == 0 && !op.fAnyOperationsAborted.as_bool() {
        Ok(())
    } else {
        Err(format!("Couldn't move to the Recycle Bin (code {r})"))
    }
}

trait WideVec {
    fn encode_wide_vec(&self) -> Vec<u16>;
}
impl WideVec for std::ffi::OsStr {
    fn encode_wide_vec(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().collect()
    }
}

/// Open Explorer with the file selected.
pub fn reveal(path: &Path) {
    let _ = std::process::Command::new("explorer.exe")
        .raw_arg(format!("/select,\"{}\"", path.display()))
        .spawn();
}

/// Open a URL or `ms-settings:` link with its handler, query string intact
/// (`explorer.exe <uri>` drops the query and opens Documents instead).
pub fn open_uri(uri: &str) {
    use windows::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    // SAFETY: plain ShellExecute with owned wide strings.
    unsafe {
        ShellExecuteW(None, &HSTRING::from("open"), &HSTRING::from(uri), None, None, SW_SHOWNORMAL);
    }
}

/// Settings › Default apps, opened on Bloop's own page (its registered
/// file types listed, with Windows' "Set default" button at the top).
pub fn open_default_apps_settings() {
    open_uri("ms-settings:defaultapps?registeredAppUser=Bloop");
}

const APP_KEY: &str = r"Software\Clients\Media\Bloop";

/// Per-user registration: Bloop shows up in "Open with" and Settings ›
/// Default apps. Windows leaves the final choice to the user.
pub fn register() -> std::io::Result<()> {
    let all: Vec<&str> = all_extensions().map(|(e, _)| e).collect();
    register_exts(&all)
}

/// Register Bloop, offering only `selected` extensions to Settings ›
/// Default apps, so its page there lists just those. "Open with" still lists
/// Bloop for every format it can open.
pub fn register_exts(selected: &[&str]) -> std::io::Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    // The installed copy when there is one, so a dev build never points
    // your file types at target\.
    let installed = crate::install_layout::installed_exe();
    let exe = if installed.exists() { installed } else { std::env::current_exe()? };
    let exe = exe.display().to_string();
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu.create_subkey(r"Software\Classes")?.0;
    for (progid, label, kind) in [
        ("Bloop.Image", "Bloop image", MediaKind::Image),
        ("Bloop.Video", "Bloop video", MediaKind::Video),
    ] {
        let (key, _) = classes.create_subkey(progid)?;
        key.set_value("", &label)?;
        key.create_subkey("DefaultIcon")?.0.set_value("", &format!("\"{exe}\",0"))?;
        key.create_subkey(r"shell\open\command")?
            .0
            .set_value("", &format!("\"{exe}\" \"%1\""))?;
        let _ = kind;
    }
    let (app, _) = classes.create_subkey(r"Applications\bloop.exe")?;
    app.set_value("FriendlyAppName", &"Bloop")?;
    app.create_subkey(r"shell\open\command")?.0.set_value("", &format!("\"{exe}\" \"%1\""))?;
    let (types, _) = app.create_subkey("SupportedTypes")?;
    let (caps, _) = hkcu.create_subkey(format!(r"{APP_KEY}\Capabilities"))?;
    caps.set_value("ApplicationName", &"Bloop")?;
    caps.set_value("ApplicationDescription", &"A fast, quiet photo, GIF and video viewer.")?;
    // Rebuilt from scratch, so formats you unticked drop off Bloop's page.
    let _ = caps.delete_subkey_all("FileAssociations");
    let (assoc, _) = caps.create_subkey("FileAssociations")?;
    for (ext, kind) in all_extensions() {
        let progid = if kind == MediaKind::Video { "Bloop.Video" } else { "Bloop.Image" };
        let dot = format!(".{ext}");
        if selected.contains(&ext) {
            assoc.set_value(&dot, &progid)?;
        }
        types.set_value(&dot, &"")?;
        let (owl, _) = classes.create_subkey(format!(r"{dot}\OpenWithProgids"))?;
        owl.set_value(progid, &"")?;
    }
    hkcu.create_subkey(r"Software\RegisteredApplications")?
        .0
        .set_value("Bloop", &format!(r"{APP_KEY}\Capabilities"))?;
    notify_assoc_changed();
    Ok(())
}

pub fn unregister() -> std::io::Result<()> {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu.open_subkey_with_flags(r"Software\Classes", winreg::enums::KEY_ALL_ACCESS)?;
    for k in ["Bloop.Image", "Bloop.Video", r"Applications\bloop.exe"] {
        let _ = classes.delete_subkey_all(k);
    }
    for (ext, kind) in all_extensions() {
        let progid = if kind == MediaKind::Video { "Bloop.Video" } else { "Bloop.Image" };
        if let Ok(k) = classes.open_subkey_with_flags(
            format!(r".{ext}\OpenWithProgids"),
            winreg::enums::KEY_ALL_ACCESS,
        ) {
            let _ = k.delete_value(progid);
        }
    }
    let _ = hkcu.delete_subkey_all(APP_KEY);
    if let Ok(k) =
        hkcu.open_subkey_with_flags(r"Software\RegisteredApplications", winreg::enums::KEY_ALL_ACCESS)
    {
        let _ = k.delete_value("Bloop");
    }
    notify_assoc_changed();
    Ok(())
}

fn notify_assoc_changed() {
    // SAFETY: documented no-argument notification.
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
}

/// Whether `.ext` currently opens with Bloop, per the user's choice in
/// Settings (UserChoice), which is what Explorer follows.
pub fn is_default_for(ext: &str) -> bool {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    let key = format!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.{ext}\UserChoice");
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(key)
        .and_then(|k| k.get_value::<String, _>("ProgId"))
        .is_ok_and(|p| p.starts_with("Bloop.") || p.eq_ignore_ascii_case(r"Applications\bloop.exe"))
}

pub fn is_registered() -> bool {
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\RegisteredApplications")
        .and_then(|k| k.get_value::<String, _>("Bloop"))
        .is_ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn thumbnail_of_a_generated_png() {
        crate::wic::com_init_sta();
        let dir = std::env::temp_dir().join("bloop-thumb-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.png");
        image::RgbaImage::from_pixel(64, 48, image::Rgba([200, 50, 50, 255])).save(&p).unwrap();
        let t = super::thumbnail(&p, 128).expect("thumbnail");
        assert!(t.width() > 0 && t.height() > 0);
    }
}

/// Remove an installed Bloop: file associations, shortcuts, the Settings ›
/// Apps entry, and finally its folder (after this process exits). Only
/// deletes the folder when running from the per-user install location.
pub fn uninstall_app() {
    use crate::install_layout as layout;
    let _ = unregister();
    for lnk in [layout::start_menu_shortcut(), layout::desktop_shortcut()] {
        let _ = std::fs::remove_file(lnk);
    }
    let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
        .delete_subkey_all(layout::UNINSTALL_KEY);
    let dir = layout::install_dir();
    let running_installed = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        .is_some_and(|p| crate::media::normalize(&p) == crate::media::normalize(&dir));
    if running_installed && dir.ends_with(r"Programs\Bloop") {
        // The exe can't delete its own folder while running; a hidden cmd
        // waits for it to exit, then removes the folder.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let _ = std::process::Command::new("cmd.exe")
            .raw_arg(format!(
                "/C ping 127.0.0.1 -n 3 > nul & rmdir /S /Q \"{}\"",
                dir.display()
            ))
            .creation_flags(CREATE_NO_WINDOW)
            .spawn();
    }
}
