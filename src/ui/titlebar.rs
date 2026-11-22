//! The custom titlebar: a drag strip with the file name and position,
//! quick actions, and Bloop's own minimise / maximise / close buttons.
use super::{
    icon_button,
    icons::{Icon, icon},
};
use crate::{app::App, theme};
use std::{rc::Rc, sync::Arc};
use zgx::{canvas::GradientStop, compose::prelude::*, scene::Color};

pub fn titlebar(app: Rc<App>) -> View {
    let scrim: Arc<[GradientStop]> = Arc::from(vec![
        GradientStop { offset: 0., color: Color(0, 0, 0, 170) },
        GradientStop { offset: 1., color: Color(0, 0, 0, 0) },
    ]);
    row()
        .absolute()
        .top(0.)
        .left(0.)
        .right(0.)
        .h(theme::TITLEBAR_H + 16.)
        .pb(16.)
        .items_center()
        .bg_gradient(std::f32::consts::FRAC_PI_2, scrim)
        .window_drag()
        .child(title(app.clone()))
        .child(div().grow())
        .child(actions(app.clone()))
        .child_when(
            {
                let app = app.clone();
                move || (!app.fullscreen.get()).then_some(())
            },
            {
                let app = app.clone();
                move |_, _| window_buttons(app.clone())
            },
        )
}

fn title(app: Rc<App>) -> View {
    row()
        .h(theme::TITLEBAR_H)
        .pl(14.)
        .gap(10.)
        .items_center()
        .min_w(0.)
        .flex_shrink(1.)
        .pointer_events(false)
        .child(logo())
        .child(
            text_signal({
                let app = app.clone();
                move || app.current().map(|e| e.name()).unwrap_or_else(|| "Bloop".into())
            })
            .text_size(13.)
            .font_weight(560)
            .truncate()
            .min_w(0.)
            .flex_shrink(1.),
        )
        .child(
            text_signal({
                let app = app.clone();
                move || {
                    let n = app.entries.with(Vec::len);
                    if n > 1 { format!("{} / {n}", app.index.get() + 1) } else { String::new() }
                }
            })
            .text_size(12.)
            .text_color(theme::TEXT_FAINT)
            .shrink_0(),
        )
}

/// The little gradient bloop.
fn logo() -> View {
    let stops: Arc<[GradientStop]> = Arc::from(vec![
        GradientStop { offset: 0., color: theme::CORAL },
        GradientStop { offset: 1., color: theme::ACCENT },
    ]);
    div().size(14., 14.).rounded(7.).bg_gradient(0.8, stops).shrink_0()
}

fn actions(app: Rc<App>) -> View {
    let a = |f: fn(&Rc<App>)| {
        let app = app.clone();
        move || f(&app)
    };
    let has_item = {
        let app = app.clone();
        move || app.current().is_some()
    };
    let has = has_item.clone();
    row()
        .h(theme::TITLEBAR_H)
        .gap(2.)
        .pr(6.)
        .items_center()
        .reactive_style(move || {
            if has() { Styles::new().opacity(1.) } else { Styles::new().opacity(0.) }
        })
        .pointer_events_when(has_item)
        .child(icon_button(Icon::ZoomOut, "Zoom out  (−)", a(|a| a.zoom_center(0.8))))
        .child(zoom_label(app.clone()))
        .child(icon_button(Icon::ZoomIn, "Zoom in  (+)", a(|a| a.zoom_center(1.25))))
        .child(sep())
        .child(icon_button(Icon::RotateRight, "Rotate  (R)", a(|a| a.rotate(1))))
        .child(icon_button(Icon::Trash, "Delete  (Del)", a(|a| a.ask_delete())))
        .child(icon_button(Icon::Info, "Info  (I)", a(|a| a.toggle_info())))
        .child(icon_button(Icon::Slideshow, "Slideshow  (S)", a(|a| a.toggle_slideshow())))
        .child(
            div()
                .child_when(
                    {
                        let app = app.clone();
                        move || Some(app.fullscreen.get())
                    },
                    {
                        let app = app.clone();
                        move |full, _| {
                            let app = app.clone();
                            if full {
                                icon_button(Icon::ExitFullscreen, "Exit full screen  (F11)", move || {
                                    app.toggle_fullscreen()
                                })
                            } else {
                                icon_button(Icon::Fullscreen, "Full screen  (F11)", move || {
                                    app.toggle_fullscreen()
                                })
                            }
                        }
                    },
                ),
        )
}

fn zoom_label(app: Rc<App>) -> View {
    let click = app.clone();
    div()
        .h(28.)
        .min_w(54.)
        .px(6.)
        .rounded(theme::RADIUS_SM)
        .items_center()
        .justify_center()
        .hover(|s| s.bg(theme::HOVER))
        .child(
            text_signal(move || format!("{:.0}%", app.view.get().scale * 100.))
                .text_size(12.)
                .text_color(theme::TEXT_DIM),
        )
        .on_click(move || {
            let (vw, vh) = click.viewport.get();
            if click.fit.get() { click.zoom_actual(vw / 2., vh / 2.) } else { click.toggle_zoom(0., 0.) }
        })
        .tooltip("Fit ↔ 100%  (0 / 1)")
}

fn sep() -> View {
    div().w(1.).h(16.).mx(4.).bg(Color(255, 255, 255, 24))
}

fn window_buttons(app: Rc<App>) -> View {
    let caption = |i: Icon, danger: bool, f: Box<dyn FnMut()>| {
        let hover = if danger { theme::CLOSE_HOVER } else { theme::HOVER };
        div()
            .w(46.)
            .h(theme::TITLEBAR_H)
            .items_center()
            .justify_center()
            .hover(move |s| s.bg(hover))
            .active(|s| s.opacity(0.8))
            .child(icon(i, 16., theme::TEXT))
            .on_click(f)
    };
    let (min, max, close) = (app.clone(), app.clone(), app.clone());
    row()
        .h(theme::TITLEBAR_H)
        .items_start()
        .child(sep())
        .child(caption(Icon::Minimize, false, Box::new(move || min.window.set_minimized(true))))
        .child(
            div().child_when(
                {
                    let app = app.clone();
                    move || Some(app.maximized.get())
                },
                move |m, _| {
                    let max = max.clone();
                    caption(
                        if m { Icon::Restore } else { Icon::Maximize },
                        false,
                        Box::new(move || max.toggle_maximize()),
                    )
                },
            ),
        )
        .child(caption(Icon::Close, true, Box::new(move || {
            close.commit_pending_delete();
            close.window.close();
        })))
}
