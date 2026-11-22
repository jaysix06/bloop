//! Chrome that floats over the picture and fades away while you look:
//! titlebar, edge arrows, the filmstrip and the video bar.
use super::{
    glass,
    icon_button,
    icons::{Icon, icon},
    titlebar::titlebar,
};
use crate::{app::App, media::MediaKind, meta::format_duration, theme};
use std::rc::Rc;
use zgx::{
    compose::prelude::*,
    input::{EventPhase, InputEvent, PointerButton},
    scene::{BoxShadow, Color, Transform},
};

/// The floating pieces, each placed on its own so the picture under the
/// empty space between them still gets the pointer.
pub fn chrome(app: Rc<App>) -> Vec<View> {
    let pieces = vec![
        hover_zone(titlebar(app.clone()), app.clone()),
        arrow(app.clone(), -1),
        arrow(app.clone(), 1),
        video_slot(app.clone()),
        hover_zone(filmstrip(app.clone()), app.clone()),
    ];
    pieces
        .into_iter()
        .map(|v| {
            let (vis, hit) = (app.clone(), app.clone());
            overlay()
                .absolute()
                .top(0.)
                .left(0.)
                .right(0.)
                .h(0.)
                .overflow_visible()
                .reactive_style(move || Styles::new().opacity(vis.chrome.get()))
                .pointer_events_when(move || hit.chrome.get() > 0.05)
                .child(v)
        })
        .collect()
}

/// Width left for the picture once the info panel takes its share.
fn area(app: &App) -> f32 {
    let (vw, _) = app.viewport.get();
    vw - if app.info_open.get() { theme::INFO_W } else { 0. }
}

/// Keeps the chrome up while the pointer rests on it.
fn hover_zone(v: View, app: Rc<App>) -> View {
    v.on_event(move |cx| {
        if cx.phase != EventPhase::Target && cx.phase != EventPhase::Bubble {
            return;
        }
        match cx.event {
            InputEvent::PointerEnter => { app.chrome_hover.set(true); }
            InputEvent::PointerLeave => { app.chrome_hover.set(false); }
            _ => {}
        }
    })
}

fn arrow(app: Rc<App>, dir: isize) -> View {
    let go = app.clone();
    let (i, tip) = if dir < 0 {
        (Icon::ChevronLeft, "Previous  (←)")
    } else {
        (Icon::ChevronRight, "Next  (→)")
    };
    let btn = div()
        .size(48., 48.)
        .rounded(24.)
        .items_center()
        .justify_center()
        .bg(theme::GLASS)
        .backdrop_blur(20.)
        .border(1.)
        .border_color(theme::GLASS_BORDER)
        .shadow(BoxShadow {
            color: theme::SHADOW,
            offset: Transform { x: 0., y: 6. },
            blur_radius: 18.,
            spread: 0.,
        })
        .hover(|s| s.bg(theme::GLASS_STRONG))
        .active(|s| s.opacity(0.85))
        .child(icon(i, 22., theme::TEXT))
        .on_click(move || go.step(dir))
        .tooltip(tip);
    let place = app.clone();
    hover_zone(
        div()
            .absolute()
            .top(0.)
            .left(0.)
            .size(48., 48.)
            .child(btn)
            .reactive_style(move || {
                let n = place.entries.with(Vec::len);
                let (_, vh) = place.viewport.get();
                let x = if dir < 0 { 20. } else { area(&place) - 20. - 48. };
                Styles::new().opacity(if n > 1 { 1. } else { 0. }).translate(x, vh / 2. - 24.)
            })
            .pointer_events_when({
                let app = app.clone();
                move || app.entries.with(Vec::len) > 1
            }),
        app,
    )
}

const STRIP_H: f32 = theme::THUMB + 12. + 2.;
const BOTTOM_PAD: f32 = 14.;

fn video_slot(app: Rc<App>) -> View {
    div().child_when(
        {
            let app = app.clone();
            move || {
                (app.kind.get() == Some(MediaKind::Video)
                    && !matches!(app.status.get(), crate::app::Status::Error(_)))
                .then_some(())
            }
        },
        move |_, _| {
            let place = app.clone();
            hover_zone(video_bar(app.clone()), app.clone()).absolute().top(0.).left(0.).reactive_style(
                move || {
                    let (_, vh) = place.viewport.get();
                    let strip = if place.entries.with(Vec::len) > 1 { STRIP_H + 10. } else { 0. };
                    Styles::new().translate(bar_left(&place), vh - BOTTOM_PAD - strip - 50.)
                },
            )
        },
    )
}

// ----- filmstrip -------------------------------------------------------------

const ITEM: f32 = theme::THUMB + 6.;

fn filmstrip(app: Rc<App>) -> View {
    let keys_app = app.clone();
    let wheel = app.clone();
    let vis = app.clone();
    let strip_w = {
        let app = app.clone();
        move || {
            let (vw, _) = app.viewport.get();
            let avail = vw - 48. - if app.info_open.get() { theme::INFO_W } else { 0. };
            let n = app.entries.with(Vec::len) as f32;
            (n * ITEM + 12.).min(avail).max(ITEM)
        }
    };
    let width = strip_w.clone();
    glass(
        overlay()
            .absolute()
            .top(0.)
            .left(0.)
            .h(theme::THUMB + 12.)
            .overflow_hidden()
            .reactive_style(move || {
                let n = vis.entries.with(Vec::len);
                let (_, vh) = vis.viewport.get();
                let w = width();
                Styles::new()
                    .w(w)
                    .opacity(if n > 1 { 1. } else { 0. })
                    .translate((area(&vis) - w) / 2., vh - BOTTOM_PAD - STRIP_H)
            })
            .child(keyed(
                move || {
                    let app = &keys_app;
                    let n = app.entries.with(Vec::len);
                    if n < 2 {
                        return Vec::new();
                    }
                    let off = app.strip_offset.get();
                    let w = strip_w();
                    let first = ((off / ITEM).floor() as isize - 4).max(0) as usize;
                    let last = (((off + w) / ITEM).ceil() as usize + 4).min(n);
                    app.entries.with(|e| {
                        (first..last).map(|i| (i, e[i].path.clone())).collect::<Vec<_>>()
                    })
                },
                {
                    let app = app.clone();
                    move |(i, path), _| thumb(app.clone(), i, path)
                },
            ))
            .on_event(move |cx| {
                if cx.phase == EventPhase::Capture {
                    return;
                }
                if let InputEvent::Scroll { delta_x, delta_y, .. } = cx.event {
                    cx.prevent_default();
                    cx.stop_propagation();
                    let d = if delta_x.abs() > delta_y.abs() { delta_x } else { delta_y };
                    let n = wheel.entries.with(Vec::len) as f32;
                    let (vw, _) = wheel.viewport.get();
                    let max = (n * ITEM + 12. - (vw - 48.)).max(0.);
                    wheel.strip_offset.set((wheel.strip_offset.get() + d).clamp(0., max));
                }
            }),
    )
}

fn thumb(app: Rc<App>, i: usize, path: std::path::PathBuf) -> View {
    let sig = app.thumb(&path);
    let is_video = crate::media::kind_of(&path) == Some(MediaKind::Video);
    let pos = app.clone();
    let click = app.clone();
    let tip = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let img = sig.clone();
    overlay()
        .absolute()
        .top(6.)
        .left(6.)
        .size(theme::THUMB, theme::THUMB)
        .rounded(7.)
        .overflow_hidden()
        .bg(Color(255, 255, 255, 10))
        .child(
            image_signal(format!("Thumbnail {}", i + 1), move || {
                img.get().unwrap_or_else(|| Arc::new(zgx::image::ImageData::new(1, 1, vec![0, 0, 0, 0]).unwrap()))
            })
            .w_full()
            .h_full()
            .object_cover()
            .pointer_events(false),
        )
        .child_when(move || is_video.then_some(()), |_, _| {
            div()
                .absolute()
                .right(4.)
                .bottom(4.)
                .size(16., 16.)
                .rounded(8.)
                .bg(Color(0, 0, 0, 150))
                .items_center()
                .justify_center()
                .child(icon(Icon::Play, 9., theme::TEXT))
        })
        .child(
            div()
                .absolute()
                .top(0.)
                .left(0.)
                .right(0.)
                .bottom(0.)
                .rounded(7.)
                .pointer_events(false)
                .reactive_style({
                    let app = app.clone();
                    move || {
                        let cur = app.index.get() == i;
                        Styles::new()
                            .border(if cur { 2. } else { 1. })
                            .border_color(if cur { theme::ACCENT } else { Color(255, 255, 255, 18) })
                    }
                }),
        )
        .hover(|s| s.opacity(1.))
        .reactive_style(move || {
            let cur = pos.index.get() == i;
            let x = i as f32 * ITEM - pos.strip_offset.get();
            Styles::new().translate(x, 0.).opacity(if cur { 1. } else { 0.62 })
        })
        .on_click(move || click.go(i, false))
        .tooltip(tip)
}

use std::sync::Arc;

// ----- video bar -------------------------------------------------------------

const BAR_MAX: f32 = 720.;
const LEFT_W: f32 = 132.;
const RIGHT_W: f32 = 168.;

fn bar_width(app: &App) -> f32 {
    let (vw, _) = app.viewport.get();
    let avail = vw - 48. - if app.info_open.get() { theme::INFO_W } else { 0. };
    avail.min(BAR_MAX).max(360.)
}

fn video_bar(app: Rc<App>) -> View {
    let w = app.clone();
    let play = app.clone();
    let mute = app.clone();
    let lp = app.clone();
    glass(
        row()
            .h(48.)
            .px(8.)
            .gap(4.)
            .items_center()
            .reactive_style(move || Styles::new().w(bar_width(&w)))
            .child(div().child_when(
                {
                    let app = app.clone();
                    move || Some(app.video_status.get().playing)
                },
                move |playing, _| {
                    let play = play.clone();
                    let (i, t) = if playing { (Icon::Pause, "Pause  (Space)") } else { (Icon::Play, "Play  (Space)") };
                    icon_button(i, t, move || play.toggle_play())
                },
            ))
            .child(
                text_signal({
                    let app = app.clone();
                    move || {
                        let s = app.video_status.get();
                        format!("{} / {}", format_duration(s.position), format_duration(s.duration))
                    }
                })
                .text_size(12.)
                .text_color(theme::TEXT_DIM)
                .w(LEFT_W - 42.)
                
                .shrink_0(),
            )
            .child(seek_bar(app.clone()))
            .child(div().child_when(
                {
                    let app = app.clone();
                    move || Some(app.muted.get() || app.volume.get() <= 0.001)
                },
                move |muted, _| {
                    let mute = mute.clone();
                    icon_button(if muted { Icon::Mute } else { Icon::Volume }, "Mute  (M)", move || {
                        mute.toggle_mute()
                    })
                },
            ))
            .child(volume_bar(app.clone()))
            .child(
                div()
                    .child(icon_button(Icon::Loop, "Loop  (L)", move || lp.toggle_loop()))
                    .reactive_style({
                        let app = app.clone();
                        move || Styles::new().opacity(if app.looping.get() { 1. } else { 0.45 })
                    }),
            ),
    )
}

/// A horizontal track that reports pointer presses and drags as 0..1.
fn track(
    width: impl Fn() -> f32 + 'static,
    left: impl Fn() -> f32 + 'static,
    value: impl Fn() -> f32 + 'static,
    mut set: impl FnMut(f32) + 'static,
) -> View {
    let width = Rc::new(width);
    let left = Rc::new(left);
    let dragging = Rc::new(std::cell::Cell::new(false));
    let (w1, w2) = (width.clone(), width.clone());
    let value = Rc::new(value);
    let v1 = value.clone();
    let hovered = Rc::new(std::cell::Cell::new(false));
    div()
        .relative()
        .h(28.)
        .shrink_0()
        .reactive_style(move || Styles::new().w(w1()))
        .child(div().absolute().left(0.).right(0.).top(12.).h(4.).rounded(2.).bg(Color(255, 255, 255, 40)))
        .child(
            div()
                .absolute()
                .left(0.)
                .top(12.)
                .h(4.)
                .rounded(2.)
                .bg(theme::TEXT)
                .reactive_style(move || Styles::new().w((v1().clamp(0., 1.) * w2()).max(0.))),
        )
        .child(
            div()
                .absolute()
                .left(0.)
                .top(8.)
                .size(12., 12.)
                .rounded(6.)
                .bg(theme::TEXT)
                .shadow(BoxShadow {
                    color: theme::SHADOW,
                    offset: Transform { x: 0., y: 1. },
                    blur_radius: 4.,
                    spread: 0.,
                })
                .reactive_style({
                    let width = width.clone();
                    move || Styles::new().translate(value().clamp(0., 1.) * width() - 6., 0.)
                }),
        )
        .on_event(move |cx| {
            if cx.phase == EventPhase::Capture {
                return;
            }
            let frac = |x: f32| ((x - left()) / width()).clamp(0., 1.);
            match cx.event {
                InputEvent::PointerDown { x, button: PointerButton::Primary, .. } => {
                    dragging.set(true);
                    cx.capture_pointer();
                    cx.stop_propagation();
                    set(frac(x));
                }
                InputEvent::PointerMove { x, .. } if dragging.get() => set(frac(x)),
                InputEvent::PointerUp { .. } | InputEvent::PointerCancel => {
                    if dragging.replace(false) {
                        cx.release_pointer();
                    }
                }
                InputEvent::PointerEnter => { hovered.set(true); }
                InputEvent::PointerLeave => { hovered.set(false); }
                _ => {}
            }
        })
}

fn bar_left(app: &App) -> f32 {
    (area(app) - bar_width(app)) / 2.
}

fn seek_bar(app: Rc<App>) -> View {
    let (a, b, c, d) = (app.clone(), app.clone(), app.clone(), app.clone());
    let seek_w = move |app: &App| bar_width(app) - 8. * 2. - LEFT_W - RIGHT_W - 16.;
    track(
        move || seek_w(&a),
        move || bar_left(&b) + 8. + LEFT_W + 8.,
        move || {
            let s = c.video_status.get();
            if s.duration > 0. { (s.position / s.duration) as f32 } else { 0. }
        },
        move |f| {
            let s = d.video_status.get();
            if s.duration > 0. {
                d.seek_to(f as f64 * s.duration);
            }
        },
    )
    .mx(8.)
}

fn volume_bar(app: Rc<App>) -> View {
    let (b, c, d) = (app.clone(), app.clone(), app.clone());
    track(
        || 72.,
        move || bar_left(&b) + bar_width(&b) - 8. - 34. - 72. - 4.,
        move || if c.muted.get() { 0. } else { c.volume.get() as f32 },
        move |f| d.set_volume(f as f64),
    )
}
