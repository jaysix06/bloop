//! Bloop Setup: a small, custom-drawn installer (no native dialogs) that
//! installs Bloop for the current user.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod install;

use install::{Options, Progress};
use std::{path::PathBuf, rc::Rc, sync::Arc};
use zgx::{
    canvas::GradientStop,
    compose::prelude::*,
    scene::{BoxShadow, Color, Transform},
};
use zgx_desktop::{Application, Titlebar, WindowOptions};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const BG: Color = Color(16, 16, 20, 255);
const TEXT: Color = Color(238, 238, 244, 255);
const DIM: Color = Color(160, 160, 172, 255);
const FAINT: Color = Color(110, 110, 122, 255);
const ACCENT: Color = Color(139, 92, 246, 255);
const ACCENT_HOVER: Color = Color(155, 112, 250, 255);
const CORAL: Color = Color(255, 122, 89, 255);
const HOVER: Color = Color(255, 255, 255, 20);
const DANGER: Color = Color(239, 68, 68, 255);

#[derive(Clone, PartialEq)]
enum Page {
    Welcome,
    Installing,
    Done(PathBuf),
    Failed(String),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // `Bloop-Setup.exe /S` installs silently, keeping the current choices
    // (the in-app updater uses this). `--relaunch --wait-pid N` waits for
    // the old Bloop to exit, then opens the new one.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a.eq_ignore_ascii_case("/S") || a == "--silent") {
        let wait_pid = args
            .iter()
            .position(|a| a == "--wait-pid")
            .and_then(|i| args.get(i + 1))
            .and_then(|p| p.parse::<u32>().ok());
        if let Some(pid) = wait_pid {
            install::wait_for_exit(pid);
        }
        let mut installed = None;
        install::run(install::silent_options(), |p| {
            if let Progress::Done(exe) = p {
                installed = Some(exe);
            }
        });
        if args.iter().any(|a| a == "--relaunch")
            && let Some(exe) = installed
        {
            let _ = std::process::Command::new(exe).spawn();
        }
        return Ok(());
    }
    Application::new()
        .window(WindowOptions {
            title: "Bloop Setup".into(),
            width: 560.,
            height: 420.,
            resizable: false,
            titlebar: Titlebar::Hidden,
            ..Default::default()
        })
        .run(|cx| {
            let rt = cx.ui.runtime.clone();
            let page = rt.signal(Page::Welcome);
            let progress = rt.signal((0f32, String::new()));
            let desktop = rt.signal(false);
            let register = rt.signal(true);
            let window = cx.window.clone();
            let tasks = cx.tasks.clone();
            let updating = install::is_installed();

            let start: Rc<dyn Fn()> = {
                let (page, progress, desktop, register) =
                    (page.clone(), progress.clone(), desktop.clone(), register.clone());
                Rc::new(move || {
                    page.set(Page::Installing);
                    let opts = Options { desktop: desktop.get(), register: register.get() };
                    let (tx, rx) = async_channel::unbounded();
                    std::thread::spawn(move || {
                        install::run(opts, |p| {
                            let _ = tx.send_blocking(p);
                        })
                    });
                    let (page, progress) = (page.clone(), progress.clone());
                    tasks.spawn(async move {
                        while let Ok(p) = rx.recv().await {
                            match p {
                                Progress::Step(f, label) => {
                                    progress.set((f, label.to_string()));
                                }
                                Progress::Done(exe) => {
                                    // Let the bar reach the end before the page changes.
                                    zgx::timer::sleep(std::time::Duration::from_millis(350)).await;
                                    page.set(Page::Done(exe));
                                }
                                Progress::Failed(e) => {
                                    page.set(Page::Failed(e));
                                }
                            }
                        }
                    });
                })
            };

            let body = {
                let (page, progress, desktop, register, window) =
                    (page.clone(), progress.clone(), desktop.clone(), register.clone(), window.clone());
                column().w_full().grow().min_h(0.).child_when(
                    move || Some(page.get()),
                    move |p, _| match p {
                        Page::Welcome => welcome(updating, desktop.clone(), register.clone(), start.clone(), window.clone()),
                        Page::Installing => installing(progress.clone()),
                        Page::Done(exe) => done(exe, updating, window.clone()),
                        Page::Failed(e) => failed(e, window.clone()),
                    },
                )
            };

            let close = window.clone();
            let closable = page.clone();
            cx.render(
                column()
                    .w_full()
                    .h_full()
                    .bg(BG)
                    .text_color(TEXT)
                    .text_size(13.)
                    .child(
                        row()
                            .h(40.)
                            .w_full()
                            .items_center()
                            .window_drag()
                            .child(text("Bloop Setup").text_size(12.).text_color(FAINT).pl(16.))
                            .child(div().grow())
                            .child(
                                div()
                                    .w(46.)
                                    .h(40.)
                                    .items_center()
                                    .justify_center()
                                    .hover(|s| s.bg(Color(196, 43, 28, 255)))
                                    .child(text("✕").text_size(12.))
                                    .on_click(move || {
                                        if closable.get() != Page::Installing {
                                            close.close();
                                        }
                                    }),
                            ),
                    )
                    .child(body),
            );
        })
}

fn logo(size: f32) -> View {
    let stops: Arc<[GradientStop]> = Arc::from(vec![
        GradientStop { offset: 0., color: CORAL },
        GradientStop { offset: 1., color: ACCENT },
    ]);
    div()
        .size(size, size)
        .rounded(size / 2.)
        .bg_gradient(0.8, stops)
        .shadow(BoxShadow {
            color: Color(139, 92, 246, 90),
            offset: Transform { x: 0., y: 10. },
            blur_radius: 32.,
            spread: 0.,
        })
}

fn button(label: &str, primary: bool, on_click: impl FnMut() + 'static) -> View {
    let (bg, hover) = if primary {
        (ACCENT, ACCENT_HOVER)
    } else {
        (Color(255, 255, 255, 16), Color(255, 255, 255, 28))
    };
    div()
        .h(36.)
        .px(20.)
        .rounded(8.)
        .items_center()
        .justify_center()
        .bg(bg)
        .hover(move |s| s.bg(hover))
        .active(|s| s.opacity(0.85))
        .child(text(label.to_string()).font_weight(580))
        .on_click(on_click)
}

/// A custom checkbox row: a rounded box that fills with the accent.
fn option(label: &str, hint: &str, value: Signal<bool>) -> View {
    let (show, flip) = (value.clone(), value.clone());
    row()
        .w_full()
        .gap(12.)
        .px(12.)
        .py(9.)
        .rounded(8.)
        .items_center()
        .hover(|s| s.bg(HOVER))
        .child(
            div()
                .size(18., 18.)
                .rounded(5.)
                .border(1.5)
                .items_center()
                .justify_center()
                .reactive_style(move || {
                    let on = show.get();
                    Styles::new()
                        .bg(if on { ACCENT } else { Color(0, 0, 0, 0) })
                        .border_color(if on { ACCENT } else { Color(255, 255, 255, 70) })
                })
                .child(text_signal({
                    let value = value.clone();
                    move || if value.get() { "✓".into() } else { String::new() }
                })
                .text_size(11.)
                .font_weight(700)),
        )
        .child(
            column()
                .gap(1.)
                .child(text(label.to_string()))
                .child(text(hint.to_string()).text_size(11.5).text_color(FAINT)),
        )
        .on_click(move || {
            flip.set(!flip.get());
        })
}

fn header(title: &str, subtitle: &str) -> View {
    column()
        .items_center()
        .gap(6.)
        .child(logo(60.))
        .child(text(title.to_string()).text_size(22.).font_weight(700).pt(12.))
        .child(text(subtitle.to_string()).text_color(DIM))
}

fn short(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    match std::env::var("USERPROFILE") {
        Ok(home) if s.starts_with(&home) => format!("%USERPROFILE%{}", &s[home.len()..]),
        _ => s,
    }
}

fn welcome(
    updating: bool,
    desktop: Signal<bool>,
    register: Signal<bool>,
    start: Rc<dyn Fn()>,
    window: zgx_desktop::WindowHandle,
) -> View {
    column()
        .w_full()
        .grow()
        .px(40.)
        .pb(24.)
        .gap(18.)
        .child(header(
            if updating { "Update Bloop" } else { "Install Bloop" },
            &format!("Fast, quiet photo, GIF and video viewer · v{}", install::VERSION),
        ))
        .child(
            column()
                .w_full()
                .gap(2.)
                .child(option("Add to \"Open with\"", "Lets you pick Bloop as your default photo and video app", register))
                .child(option("Desktop shortcut", "The Start menu shortcut is always added", desktop)),
        )
        .child(div().grow())
        .child(
            row()
                .w_full()
                .items_center()
                .gap(10.)
                .child(
                    column()
                        .grow()
                        .min_w(0.)
                        .gap(2.)
                        .child(text("Installs for you only. No admin needed.").text_size(11.5).text_color(FAINT))
                        .child(text(short(&install::layout::install_dir())).text_size(11.5).text_color(FAINT)),
                )
                .child(button("Cancel", false, move || window.close()))
                .child(button(if updating { "Update" } else { "Install" }, true, move || start())),
        )
}

fn installing(progress: Signal<(f32, String)>) -> View {
    let (bar, label) = (progress.clone(), progress.clone());
    column()
        .w_full()
        .grow()
        .px(56.)
        .gap(18.)
        .items_center()
        .justify_center()
        .child(logo(60.))
        .child(text("Installing Bloop…").text_size(18.).font_weight(650).pt(8.))
        .child(
            div()
                .w(360.)
                .h(6.)
                .rounded(3.)
                .bg(Color(255, 255, 255, 24))
                .child(
                    div()
                        .absolute()
                        .left(0.)
                        .top(0.)
                        .h(6.)
                        .rounded(3.)
                        .bg(ACCENT)
                        .reactive_style(move || Styles::new().w(360. * bar.get().0.clamp(0.02, 1.))),
                ),
        )
        .child(text_signal(move || label.get().1).text_color(DIM).text_size(12.))
        .child(div().h(40.))
}

fn done(exe: PathBuf, updated: bool, window: zgx_desktop::WindowHandle) -> View {
    let (close, launch) = (window.clone(), window.clone());
    column()
        .w_full()
        .grow()
        .px(40.)
        .pb(24.)
        .gap(18.)
        .child(header(
            if updated { "Bloop is up to date" } else { "Bloop is installed" },
            "Find it in the Start menu, or open any photo with it.",
        ))
        .child(
            column()
                .w_full()
                .gap(6.)
                .px(12.)
                .child(text("Tip: to make Bloop your default viewer, open Settings › Apps › Default apps and pick Bloop.").text_size(12.).text_color(DIM).text_wrap(true).w_full())
                .child(text("To uninstall, use Settings › Apps › Installed apps.").text_size(12.).text_color(FAINT)),
        )
        .child(div().grow())
        .child(
            row()
                .w_full()
                .gap(10.)
                .justify_end()
                .child(button("Close", false, move || close.close()))
                .child(button("Launch Bloop", true, move || {
                    let _ = std::process::Command::new(&exe).spawn();
                    launch.close();
                })),
        )
}

fn failed(error: String, window: zgx_desktop::WindowHandle) -> View {
    column()
        .w_full()
        .grow()
        .px(40.)
        .pb(24.)
        .gap(14.)
        .items_center()
        .child(div().h(20.))
        .child(
            div()
                .size(56., 56.)
                .rounded(28.)
                .bg(Color(239, 68, 68, 40))
                .items_center()
                .justify_center()
                .child(text("!").text_size(26.).font_weight(700).text_color(DANGER)),
        )
        .child(text("Setup couldn't finish").text_size(20.).font_weight(700))
        .child(text(error).text_color(DIM).text_wrap(true).w(440.).text_align(TextAlign::Center))
        .child(div().grow())
        .child(row().w_full().justify_end().child(button("Close", true, move || window.close())))
}

use zgx::reactive::Signal;
