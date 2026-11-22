//! The window's views. Everything is custom-drawn: titlebar, menus,
//! dialogs and the file browser are zgx views, never native controls.
mod browser;
mod controls;
mod default_dialog;
pub mod icons;
mod panels;
mod titlebar;
mod update_card;
mod viewer;

use crate::{app::App, theme};
use icons::{Icon, icon};
use std::rc::Rc;
use zgx::{
    compose::prelude::*,
    input::{EventPhase, InputEvent, Key},
    platform::PlatformService,
    scene::Color,
};

pub fn root(app: Rc<App>) -> View {
    let keys = app.clone();
    let events = app.clone();
    let platform = app.clone();
    component(move |cx| {
        *platform.platform.borrow_mut() = cx.try_service::<PlatformService>();
        overlay()
            .w_full()
            .h_full()
            .bg(theme::STAGE)
            .text_color(theme::TEXT)
            .text_size(13.)
            .child(viewer::stage(app.clone()))
            .children(controls::chrome(app.clone()))
            .child(panels::info_panel(app.clone()))
            .child_when(
                {
                    let app = app.clone();
                    move || app.browser_open.get().then_some(())
                },
                {
                    let app = app.clone();
                    move |_, _| browser::browser(app.clone())
                },
            )
            .child(update_card::update_card(app.clone()))
            .child(panels::drop_hint(app.clone()))
            .child(panels::confirm_delete(app.clone()))
            .child(panels::help(app.clone()))
            .child(default_dialog::default_dialog(app.clone()))
            .child(zgx_components::toast_host())
            .on_document_event(move |event| on_key(&keys, event))
            .on_event(move |cx| {
                if cx.phase == EventPhase::Capture {
                    return;
                }
                match &cx.event {
                    InputEvent::FileHover { .. } => { events.drop_hover.set(true); }
                    InputEvent::FileHoverCancelled => { events.drop_hover.set(false); }
                    InputEvent::FilesDrop { paths, .. } => {
                        events.drop_hover.set(false);
                        if let Some(first) = paths.first() {
                            events.open_path(first);
                        }
                        cx.prevent_default();
                    }
                    _ => {}
                }
            })
    })
}

fn on_key(app: &Rc<App>, event: &InputEvent) {
    match event {
        InputEvent::PointerMove { .. } => app.activity(),
        InputEvent::KeyDown { key, modifiers, repeat } => {
            let ctrl = modifiers.control;
            let shift = modifiers.shift;
            // While typing a path in the browser, only Escape is ours.
            if app.browser_open.get() {
                if *key == Key::Escape {
                    app.escape();
                }
                return;
            }
            if app.default_open.get() {
                if *key == Key::Escape {
                    app.close_default_dialog();
                }
                return;
            }
            if app.confirm_open.get() {
                match key {
                    Key::Escape => { app.confirm_open.set(false); }
                    Key::Enter | Key::Delete if !repeat => app.delete_current(),
                    _ => {}
                }
                return;
            }
            let video = app.kind.get() == Some(crate::media::MediaKind::Video);
            let ch = match key {
                Key::Character(c) => c.to_lowercase(),
                _ => String::new(),
            };
            match key {
                Key::ArrowLeft if video && shift => app.seek_by(-5.),
                Key::ArrowRight if video && shift => app.seek_by(5.),
                Key::ArrowLeft | Key::PageUp => {
                    if app.is_zoomed() && ctrl {
                        app.pan_by(120., 0.)
                    } else {
                        app.step(-1)
                    }
                }
                Key::ArrowRight | Key::PageDown => {
                    if app.is_zoomed() && ctrl {
                        app.pan_by(-120., 0.)
                    } else {
                        app.step(1)
                    }
                }
                Key::ArrowUp if app.is_zoomed() => app.pan_by(0., 120.),
                Key::ArrowDown if app.is_zoomed() => app.pan_by(0., -120.),
                Key::ArrowUp if video => app.set_volume(app.volume.get() + 0.05),
                Key::ArrowDown if video => app.set_volume(app.volume.get() - 0.05),
                Key::Home => app.first(),
                Key::End => app.last(),
                Key::Escape => {
                    app.escape();
                }
                Key::Function(11) => app.toggle_fullscreen(),
                Key::Function(1) => { app.help_open.set(!app.help_open.get()); }
                Key::Delete if !repeat => app.ask_delete(),
                Key::Space if !repeat => {
                    if video {
                        app.toggle_play()
                    } else if app.slideshow.get() || !app.entries.with(Vec::is_empty) {
                        app.toggle_slideshow()
                    }
                }
                Key::Enter if !repeat => app.toggle_fullscreen(),
                Key::Character(_) if !repeat || matches!(ch.as_str(), "+" | "=" | "-") => {
                    match (ch.as_str(), ctrl, shift) {
                        ("a", false, _) => app.step(-1),
                        ("d", false, _) => app.step(1),
                        ("f", false, _) => app.toggle_fullscreen(),
                        ("i", false, _) => app.toggle_info(),
                        ("r", false, false) => app.rotate(1),
                        ("r", false, true) => app.rotate(-1),
                        ("r", true, _) | ("s", true, _) => app.save_rotation(),
                        ("s", false, _) => app.toggle_slideshow(),
                        ("m", false, _) => app.toggle_mute(),
                        ("l", false, _) if video => app.toggle_loop(),
                        ("c", true, false) => app.copy_image(),
                        ("c", true, true) => app.copy_path(),
                        ("e", true, _) => app.reveal(),
                        ("o", true, _) => { app.browser_open.set(true); }
                        ("?", _, _) | ("/", _, true) => { app.help_open.set(!app.help_open.get()); }
                        ("+" | "=", _, _) => app.zoom_center(1.25),
                        ("-" | "_", _, _) => app.zoom_center(0.8),
                        ("0", _, _) => {
                            if !app.fit.get() {
                                app.toggle_zoom(0., 0.)
                            }
                        }
                        ("1", _, _) => {
                            let (vw, vh) = app.viewport.get();
                            app.zoom_actual(vw / 2., vh / 2.)
                        }
                        ("j", false, _) if video => app.seek_by(-10.),
                        ("k", false, _) if video => app.toggle_play(),
                        (",", false, _) if video => app.seek_by(-1. / 30.),
                        (".", false, _) if video => app.seek_by(1. / 30.),
                        _ => return,
                    }
                }
                _ => return,
            }
            app.activity();
        }
        _ => {}
    }
}

/// A square, borderless icon button with a soft hover.
pub fn icon_button(i: Icon, tip: &str, on_click: impl FnMut() + 'static) -> View {
    div()
        .size(34., 32.)
        .rounded(theme::RADIUS_SM)
        .items_center()
        .justify_center()
        .hover(|s| s.bg(theme::HOVER))
        .active(|s| s.bg(theme::PRESS))
        .child(icon(i, 18., theme::TEXT))
        .on_click(on_click)
        .tooltip(tip.to_string())
}

/// A text button: `primary` is filled with the accent.
pub fn text_button(label: &str, primary: bool, on_click: impl FnMut() + 'static) -> View {
    let (bg, hover) = if primary {
        (theme::ACCENT, Color(155, 112, 250, 255))
    } else {
        (Color(255, 255, 255, 16), Color(255, 255, 255, 28))
    };
    div()
        .h(34.)
        .px(16.)
        .rounded(theme::RADIUS_SM)
        .items_center()
        .justify_center()
        .bg(bg)
        .hover(move |s| s.bg(hover))
        .active(|s| s.opacity(0.85))
        .child(text(label.to_string()).text_size(13.).font_weight(560))
        .on_click(on_click)
}

/// The frosted panel look shared by the chrome.
pub fn glass(v: View) -> View {
    v.bg(theme::GLASS)
        .backdrop_blur(24.)
        .border(1.)
        .border_color(theme::GLASS_BORDER)
        .rounded(theme::RADIUS)
}
