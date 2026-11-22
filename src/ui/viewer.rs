//! The stage: the picture (or video frame), zoom and pan, the cross-fade
//! from the previous item, and the loading / error / empty states.
use super::icons::{Icon, icon};
use crate::{
    app::{App, Status},
    media::MediaKind,
    theme,
};
use std::{cell::Cell, rc::Rc};
use zgx::{
    compose::prelude::*,
    image::ImageRendering,
    input::{EventPhase, InputEvent, PointerButton},
    scene::Color,
};

/// One notch of a Windows wheel is 100 points; zoom 18% per notch.
const NOTCH: f32 = 100.;
const ZOOM_PER_NOTCH: f32 = 1.18;

pub fn stage(app: Rc<App>) -> View {
    let swipe = Rc::new(Cell::new(0f32));
    let ev = app.clone();
    let menu = app.clone();
    overlay()
        .w_full()
        .h_full()
        .overflow_hidden()
        .child(previous_layer(app.clone()))
        .child(current_layer(app.clone()))
        .child(edge_flash(app.clone()))
        .child_when(
            {
                let app = app.clone();
                move || match app.status.get() {
                    Status::Loading => Some(0),
                    Status::Error(_) => Some(1),
                    Status::Empty => Some(2),
                    Status::Ready => None,
                }
            },
            {
                let app = app.clone();
                move |which, _| match which {
                    0 => loading(app.clone()),
                    1 => error(app.clone()),
                    _ => super::browser::empty_state(app.clone()),
                }
            },
        )
        .on_event(move |cx| {
            if cx.phase == EventPhase::Capture {
                return;
            }
            let app = &ev;
            match cx.event.clone() {
                InputEvent::PointerDown { x, y, button: PointerButton::Primary } => {
                    if app.status.get() != Status::Ready {
                        return;
                    }
                    if cx.click_count() >= 2 {
                        app.drag.set(None);
                        if app.kind.get() == Some(MediaKind::Video) {
                            app.toggle_fullscreen();
                        } else {
                            app.toggle_zoom(x, y);
                        }
                    } else if app.is_zoomed() {
                        app.drag.set(Some((x, y, app.view.get())));
                        cx.capture_pointer();
                    } else if app.kind.get() == Some(MediaKind::Video) {
                        app.toggle_play();
                    }
                }
                InputEvent::PointerDown { button: PointerButton::Middle, .. } => {
                    app.toggle_fullscreen();
                }
                InputEvent::PointerDown { button: PointerButton::Other(n), .. } => match n {
                    // Mouse back/forward buttons.
                    3 => app.step(-1),
                    4 => app.step(1),
                    _ => {}
                },
                InputEvent::PointerMove { x, y } => {
                    if let Some((sx, sy, from)) = app.drag.get() {
                        app.pan_to(from.x + x - sx, from.y + y - sy, from);
                    }
                }
                InputEvent::PointerUp { .. } | InputEvent::PointerCancel => {
                    if app.drag.take().is_some() {
                        cx.release_pointer();
                    }
                }
                InputEvent::Scroll { x, y, delta_x, delta_y } => {
                    cx.prevent_default();
                    let mods = cx.pointer_modifiers();
                    let horizontal = if mods.shift { delta_y } else { delta_x };
                    if horizontal.abs() > delta_y.abs() * 0.5 || mods.shift {
                        // Sideways scroll / swipe steps through the folder.
                        let acc = swipe.get() + horizontal;
                        if acc.abs() >= NOTCH {
                            app.step(if acc > 0. { 1 } else { -1 });
                            swipe.set(0.);
                        } else {
                            swipe.set(acc);
                        }
                        return;
                    }
                    if app.status.get() == Status::Ready {
                        let factor = ZOOM_PER_NOTCH.powf(-delta_y / NOTCH);
                        app.zoom_at(factor, x, y);
                    }
                }
                InputEvent::Pinch { x, y, delta } => {
                    cx.prevent_default();
                    app.zoom_at(1. + delta, x, y);
                }
                _ => {}
            }
        })
        .context_menu(move |_| context_items(&menu))
}

fn context_items(app: &Rc<App>) -> Vec<View> {
    if app.current().is_none() {
        return vec![
            menu_item("Open folder…").shortcut("Ctrl+O").on_click({
                let app = app.clone();
                move || { app.browser_open.set(true); }
            }),
            menu_item("Make Bloop as Default…").on_click({
                let app = app.clone();
                move || app.open_default_dialog()
            }),
            menu_item("Check for updates…").on_click({
                let app = app.clone();
                move || app.check_updates(true)
            }),
        ];
    }
    let is_image = app.kind.get() == Some(MediaKind::Image);
    let mut items = Vec::new();
    let a = |f: fn(&Rc<App>)| {
        let app = app.clone();
        move || f(&app)
    };
    if is_image {
        items.push(menu_item("Rotate right").shortcut("R").on_click(a(|a| a.rotate(1))));
        items.push(menu_item("Rotate left").shortcut("Shift+R").on_click(a(|a| a.rotate(-1))));
        if app.has_unsaved_rotation() {
            items.push(menu_item("Save rotation").shortcut("Ctrl+S").on_click(a(|a| a.save_rotation())));
        }
    }
    if app.kind.get() != Some(MediaKind::Video) {
        items.push(menu_item("Copy picture").shortcut("Ctrl+C").on_click(a(|a| a.copy_image())));
    }
    items.push(menu_item("Copy path").shortcut("Ctrl+Shift+C").on_click(a(|a| a.copy_path())));
    items.push(menu_item("Show in folder").shortcut("Ctrl+E").on_click(a(|a| a.reveal())));
    items.push(menu_item("Open another…").shortcut("Ctrl+O").on_click(a(|a| { a.browser_open.set(true); })));
    items.push(menu_item("Slideshow").shortcut("S").on_click(a(|a| a.toggle_slideshow())));
    items.push(menu_item("Info").shortcut("I").on_click(a(|a| a.toggle_info())));
    items.push(menu_item("Delete").shortcut("Del").on_click(a(|a| a.ask_delete())));
    items.push(menu_item("Make Bloop as Default…").on_click(a(|a| a.open_default_dialog())));
    items.push(menu_item("Check for updates…").on_click(a(|a| a.check_updates(true))));
    items
}

fn current_layer(app: Rc<App>) -> View {
    let src = app.clone();
    image_signal("Picture", move || src.image.get())
        .absolute()
        .left(0.)
        .top(0.)
        .object_fit(ObjectFit::Fill)
        .pointer_events(false)
        .reactive_style(move || {
            let v = app.view.get();
            let (iw, ih) = app.size.get();
            let (vw, vh) = app.viewport.get();
            let (w, h) = (iw * v.scale, ih * v.scale);
            let shown = app.status.get() == Status::Ready;
            Styles::new()
                .w(w.max(1.))
                .h(h.max(1.))
                .translate((vw - w) / 2. + v.x, (vh - h) / 2. + v.y)
                .opacity(if shown { app.fade.get() } else { 0. })
                .image_rendering(if v.scale >= 3. {
                    ImageRendering::Pixelated
                } else {
                    ImageRendering::Auto
                })
        })
}

fn previous_layer(app: Rc<App>) -> View {
    let src = app.clone();
    image_signal("Previous picture", move || src.prev_image.get())
        .absolute()
        .left(0.)
        .top(0.)
        .object_fit(ObjectFit::Fill)
        .pointer_events(false)
        .reactive_style(move || {
            let (w, h) = app.prev_size.get();
            let (vw, vh) = app.viewport.get();
            let f = app.fade.get();
            // Fade out quicker than the new one fades in.
            let o = if f >= 1. { 0. } else { (1. - f * 1.6).max(0.) };
            Styles::new().w(w.max(1.)).h(h.max(1.)).translate((vw - w) / 2., (vh - h) / 2.).opacity(o)
        })
}

/// A soft glow at the edge when the carousel wraps around.
fn edge_flash(app: Rc<App>) -> View {
    div()
        .absolute()
        .top(0.)
        .bottom(0.)
        .left(0.)
        .right(0.)
        .pointer_events(false)
        .reactive_style(move || {
            Styles::new()
                .opacity(app.edge_flash.get() * 0.6)
                .inset_shadows(vec![zgx::scene::BoxShadow {
                    color: Color(139, 92, 246, 120),
                    offset: zgx::scene::Transform { x: 0., y: 0. },
                    blur_radius: 80.,
                    spread: 0.,
                }])
        })
}

fn loading(app: Rc<App>) -> View {
    // Appears only if loading takes a moment, so quick loads never flash it.
    let shown = app.rt.signal(0f32);
    let s = shown.clone();
    let tasks = app.tasks.clone();
    tasks.spawn(async move {
        zgx::timer::sleep(std::time::Duration::from_millis(220)).await;
        for i in 1..=10 {
            s.set(i as f32 / 10.);
            zgx::timer::sleep(std::time::Duration::from_millis(16)).await;
        }
    });
    column()
        .absolute()
        .w_full()
        .h_full()
        .items_center()
        .justify_center()
        .gap(14.)
        .pointer_events(false)
        .reactive_style(move || Styles::new().opacity(shown.get()))
        .child(spinner(app.clone()))
        .child(
            text_signal({
                let app = app.clone();
                move || app.current().map(|e| e.name()).unwrap_or_default()
            })
            .text_color(theme::TEXT_FAINT)
            .text_size(12.),
        )
}

fn spinner(app: Rc<App>) -> View {
    let angle = app.rt.signal(0f32);
    let a = angle.clone();
    let start = std::time::Instant::now();
    let weak = Rc::downgrade(&app);
    app.tasks.spawn(async move {
        while weak.upgrade().is_some_and(|app| app.status.get() == Status::Loading) {
            a.set(start.elapsed().as_secs_f32() * 6.);
            zgx::timer::sleep(std::time::Duration::from_millis(16)).await;
        }
    });
    overlay()
        .size(28., 28.)
        .child(icon(Icon::Ring, 28., Color(255, 255, 255, 40)))
        .child(
            icon(Icon::Arc, 28., theme::ACCENT)
                .reactive_style(move || Styles::new().rotate(angle.get())),
        )
}

fn error(app: Rc<App>) -> View {
    column()
        .absolute()
        .w_full()
        .h_full()
        .items_center()
        .justify_center()
        .gap(10.)
        .child(icon(
            if app.kind.get() == Some(MediaKind::Video) { Icon::Video } else { Icon::Image },
            40.,
            theme::TEXT_FAINT,
        ))
        .child(
            text_signal({
                let app = app.clone();
                move || app.current().map(|e| e.name()).unwrap_or_default()
            })
            .text_size(15.)
            .font_weight(600),
        )
        .child(
            text_signal({
                let app = app.clone();
                move || match app.status.get() {
                    Status::Error(e) => friendly(&e, app.current().map(|c| c.path)),
                    _ => String::new(),
                }
            })
            .text_color(theme::TEXT_DIM)
            .max_w(460.)
            .text_wrap(true)
            .text_align(TextAlign::Center),
        )
}

fn friendly(e: &str, path: Option<std::path::PathBuf>) -> String {
    let ext = path
        .and_then(|p| p.extension().map(|x| x.to_string_lossy().to_lowercase()))
        .unwrap_or_default();
    if e == "codec" {
        return match ext.as_str() {
            "webm" | "mkv" => "Windows can't play this video yet. Microsoft's free \"Web Media Extensions\" (and VP9 / AV1 extensions) from the Microsoft Store add support.".into(),
            "mov" | "mp4" | "m4v" => "Windows can't decode this video. The free HEVC or AV1 Video Extensions from the Microsoft Store may add support.".into(),
            _ => "Windows doesn't have a decoder for this video format.".into(),
        };
    }
    if e.contains("Windows has no decoder") {
        "Bloop couldn't read this picture. If it's AVIF or HEIC, the free AV1 or HEIF extension from the Microsoft Store adds support.".into()
    } else {
        format!("Bloop couldn't open this file. {e}")
    }
}
