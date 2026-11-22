# Bloop

A fast, quiet photo, GIF and video viewer for Windows, written in Rust on
[zgx](https://github.com/zeronsh/zgx). Open one file and the whole folder
becomes a carousel. Every part of the window is drawn by Bloop: the
titlebar, menus, dialogs and file browser are not native Windows controls.

## Install

Run `dist\Bloop-Setup-<version>.exe`. It installs for the current user
only (no admin) into `%LOCALAPPDATA%\Programs\Bloop`, and adds:

- a Start menu shortcut, and optionally a desktop one
- Bloop to **Open with** and **Settings › Default apps**
- an entry in **Settings › Apps › Installed apps** for uninstalling

`Bloop-Setup.exe /S` installs silently with the defaults. Running the setup
again updates Bloop in place, even while it's open.

To build the installer:

```powershell
.\scripts\package.ps1
```

It builds `bloop.exe`, embeds it in the setup program (`installer/`), and
writes `dist\Bloop-Setup-<version>.exe`. Both executables link the C runtime
statically, so they need nothing else installed.

## Updates

An installed Bloop checks this repo's
[GitHub Releases](https://github.com/jaysix06/bloop/releases) about five
seconds after launch, at most once a day. When there is a newer version, it
downloads `Bloop-Setup-<version>.exe` in the background and verifies it
against the release's `.sha256` file. A card then offers **Restart**, or
installs the update silently the next time you close Bloop. You can also
pick **Check for updates…** from the right-click menu or the Details panel.
Copies run from `target\` never update themselves.

## Releasing

```powershell
.\scripts\release.ps1 -Version 0.2.0 -Notes "What changed"
```

The script bumps the version (shared by the app and the installer in
`[workspace.package]`), commits, builds the installer and its checksum,
pushes, and publishes the `v0.2.0` release with both files attached. You
need the GitHub CLI, signed in.

## Build and run

```powershell
cargo build --release
.\target\release\bloop.exe C:\path\to\photo.jpg
```

You need stable Rust 1.95 or newer on the MSVC toolchain, plus the Visual
Studio C++ build tools.

To make Bloop show up in **Open with** and **Settings › Default apps**
(per-user, no admin rights needed):

```powershell
.\target\release\bloop.exe --register
```

`--unregister` removes it.

To make Bloop your default viewer, right-click and choose **Make Bloop as
Default…** (also in the Details panel, and offered once on first run). Pick
the formats you want. Bloop registers for exactly those and opens Settings ›
Default apps on its own page, where Windows has you confirm them. Windows
doesn't let apps set defaults themselves. The dialog shows live which
formats already open with Bloop.

## Formats

- **Images:** JPG, PNG, WebP, BMP, TIFF, ICO, TGA, SVG and animated GIF.
  AVIF, HEIC and JPEG XR open through Windows' own codecs, so they need the
  matching free Microsoft Store extensions.
- **Video:** MP4, MOV, M4V, AVI, WMV and MKV/WebM, played through Windows
  Media Foundation. WebM needs Microsoft's free *Web Media Extensions*.

## Keys

| Key | Action |
| --- | --- |
| ← → · A D · PgUp PgDn | Previous / next (wraps around) |
| Home / End | First / last |
| Wheel · pinch | Zoom toward the pointer |
| Shift+wheel · sideways swipe | Previous / next |
| Double-click | Fit ↔ 100% |
| + − · 0 · 1 | Zoom in, zoom out · fit · 100% |
| Drag | Pan while zoomed |
| F · F11 · Enter | Full screen |
| R · Shift+R · Ctrl+S | Rotate right, rotate left · save the rotation (lossless for camera JPEGs) |
| I | Details panel (EXIF) |
| Del | Move to Recycle Bin, with Undo |
| S | Slideshow |
| Space · K | Play / pause video |
| Shift+← → · J | Seek video |
| M · L | Mute · loop |
| Ctrl+C · Ctrl+Shift+C | Copy picture · copy path |
| Ctrl+E | Show in folder |
| Ctrl+O | Open (built-in browser) |
| F1 | All shortcuts |
| Esc | Close whatever is on top |

## Layout

- `src/app.rs`: state and actions (navigation, zoom/pan, video, delete with undo)
- `src/loader.rs`: background decoding and an LRU cache of neighbouring pictures
- `src/decode.rs`, `src/wic.rs`: image decoding, using the `image` crate first and WIC as the fallback
- `src/video.rs`: Media Foundation Media Engine, which hands decoded frames to the UI
- `src/shell.rs`: Explorer thumbnails, Recycle Bin, file associations
- `src/ui/`: the views (stage, chrome, panels, browser, icons)
