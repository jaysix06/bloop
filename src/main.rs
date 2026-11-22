//! Bloop: a fast, quiet photo, GIF and video viewer for Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod decode;
mod install_layout;
mod loader;
mod media;
mod meta;
mod orient;
mod prefs;
mod recent;
mod shell;
mod theme;
mod ui;
mod update;
mod video;
mod wic;

use std::{cell::RefCell, path::PathBuf, rc::Rc};
use zgx_desktop::{Application, ApplicationEvent, Titlebar, WindowOptions};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

thread_local! {
    /// The window's app, for files a second launch hands over.
    static APP: RefCell<Option<Rc<app::App>>> = const { RefCell::new(None) };
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quiet = args.iter().any(|a| a == "--quiet");
    if args.iter().any(|a| a == "--register") {
        shell::register()?;
        if !quiet {
            println!("Bloop registered for this user. Choose it in Settings › Apps › Default apps.");
            shell::open_default_apps_settings();
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--uninstall") {
        shell::uninstall_app();
        return Ok(());
    }
    if args.iter().any(|a| a == "--unregister") {
        shell::unregister()?;
        println!("Bloop's file associations were removed.");
        return Ok(());
    }
    let initial: Option<PathBuf> = args.iter().find(|a| !a.starts_with("--")).map(PathBuf::from);

    Application::new()
        // One Bloop at a time: opening another file reuses this window.
        .application_id(install_layout::APP_ID)?
        .on_application_event(|event, _| {
            if let ApplicationEvent::OpenUrls(urls) = event {
                APP.with(|a| {
                    if let Some(app) = a.borrow().as_ref() {
                        if let Some(path) = urls.iter().find_map(|u| media::path_from_url(u)) {
                            app.open_path(&path);
                            app.window.set_minimized(false);
                            app.window.request_focus();
                        }
                    }
                });
            }
        })
        .window(WindowOptions {
            title: "Bloop".into(),
            width: 1200.,
            height: 800.,
            min_size: Some((520., 360.)),
            titlebar: Titlebar::Hidden,
            ..Default::default()
        })
        .run(move |cx| {
            cx.ui.set_zoomable(false);
            let app = app::App::new(
                cx.ui.runtime.clone(),
                cx.window.clone(),
                cx.tasks.clone(),
                cx.viewport.clone(),
            );
            APP.with(|a| *a.borrow_mut() = Some(app.clone()));
            // Keep the picture fitted as the window resizes.
            {
                let app = app.clone();
                let viewport = cx.viewport.clone();
                let effect = cx.ui.runtime.effect(move || {
                    let _ = viewport.get();
                    let _ = app.size.get();
                    app.refit_untracked();
                });
                std::mem::forget(effect);
            }
            {
                let app = app.clone();
                let fullscreen = cx.window_info.fullscreen.clone();
                let effect = cx.ui.runtime.effect(move || {
                    let f = fullscreen.get();
                    if app.fullscreen.with_untracked(|v| *v) != f {
                        app.fullscreen.set(f);
                    }
                });
                std::mem::forget(effect);
            }
            {
                let app = app.clone();
                cx.on_close_requested(move || {
                    app.on_exit();
                    true
                });
            }
            match &initial {
                Some(path) => app.open_path(path),
                None => { app.status.set(app::Status::Empty); }
            }
            cx.render(ui::root(app));
        })
}
