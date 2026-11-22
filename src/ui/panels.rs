//! Side panel and dialogs, all drawn by Bloop.
use super::{
    icon_button,
    icons::{Icon, icon},
    text_button,
};
use crate::{app::App, theme};
use std::rc::Rc;
use zgx::{
    compose::prelude::*,
    scene::{BoxShadow, Color, Transform},
};

pub fn info_panel(app: Rc<App>) -> View {
    let slide = app.clone();
    let close = app.clone();
    column()
        .absolute()
        .top(0.)
        .right(0.)
        .bottom(0.)
        .w(theme::INFO_W)
        .bg(theme::GLASS_STRONG)
        .backdrop_blur(30.)
        .border_edges_left()
        .reactive_style(move || {
            let open = slide.info_open.get();
            Styles::new()
                .translate(if open { 0. } else { theme::INFO_W + 20. }, 0.)
                .opacity(if open { 1. } else { 0. })
        })
        .pointer_events_when({
            let app = app.clone();
            move || app.info_open.get()
        })
        .child(
            row()
                .h(theme::TITLEBAR_H + 8.)
                .px(18.)
                .pt(8.)
                .items_center()
                .window_drag()
                .child(text("Details").text_size(15.).font_weight(640))
                .child(div().grow())
                .child(icon_button(Icon::Close, "Close  (I)", move || { close.info_open.set(false); })),
        )
        .child(
            scroll(app.rt.signal(0.))
                .grow()
                .min_h(0.)
                .child(column().px(18.).pb(18.).gap(18.).child(info_body(app.clone()))),
        )
        .child(register_footer(app.clone()))
}

fn info_body(app: Rc<App>) -> View {
    column().gap(18.).child_when(
        {
            let app = app.clone();
            move || app.meta.get()
        },
        {
            let app = app.clone();
            move |meta, _| {
                let name = app.current().map(|e| e.name()).unwrap_or_default();
                let mut col = column()
                    .gap(18.)
                    .child(text(name).text_size(14.).font_weight(600).text_wrap(true).w_full());
                col = col.child(section("File", &meta.rows));
                if !meta.camera.is_empty() {
                    col = col.child(section("Camera", &meta.camera));
                }
                col
            }
        },
    )
}

fn section(title: &str, rows: &[(String, String)]) -> View {
    let mut col = column().gap(10.).child(
        text(title.to_uppercase())
            .text_size(10.5)
            .letter_spacing(0.8)
            .font_weight(640)
            .text_color(theme::TEXT_FAINT),
    );
    for (k, v) in rows {
        col = col.child(
            column()
                .gap(2.)
                .child(text(k.clone()).text_size(11.5).text_color(theme::TEXT_DIM))
                .child(text(v.clone()).text_size(13.).text_wrap(true).w_full().selectable(true)),
        );
    }
    col
}

fn register_footer(app: Rc<App>) -> View {
    let check = app.clone();
    column().px(18.).pb(18.).pt(12.).gap(10.).child(
        row()
            .gap(8.)
            .items_center()
            .child(text(format!("Bloop {}", crate::update::CURRENT)).text_size(12.).text_color(theme::TEXT_FAINT))
            .child(div().grow())
            .child(
                text("Check for updates")
                    .text_size(12.)
                    .text_color(theme::ACCENT)
                    .on_click(move || check.check_updates(true)),
            ),
    ).child_when(
        {
            let app = app.clone();
            move || Some(app.registered.get())
        },
        {
            let app = app.clone();
            move |registered, _| {
                let app = app.clone();
                if registered {
                    row()
                        .gap(8.)
                        .items_center()
                        .child(icon(Icon::Check, 15., theme::TEXT_DIM))
                        .child(text("Bloop is in Windows' Open with list").text_size(12.).text_color(theme::TEXT_DIM))
                        .child(div().grow())
                        .child(
                            text("Defaults…")
                                .text_size(12.)
                                .text_color(theme::ACCENT)
                                .on_click(move || app.open_default_dialog()),
                        )
                } else {
                    text_button("Make Bloop as Default…", false, move || app.open_default_dialog())
                }
            }
        },
    )
}

pub fn drop_hint(app: Rc<App>) -> View {
    let vis = app.clone();
    column()
        .absolute()
        .top(10.)
        .left(10.)
        .right(10.)
        .bottom(10.)
        .rounded(18.)
        .border(2.)
        .border_color(theme::ACCENT)
        .bg(theme::ACCENT_SOFT)
        .items_center()
        .justify_center()
        .gap(10.)
        .pointer_events(false)
        .reactive_style(move || Styles::new().opacity(if vis.drop_hover.get() { 1. } else { 0. }))
        .child(icon(Icon::Image, 44., theme::TEXT))
        .child(text("Drop to open").text_size(18.).font_weight(600))
}

fn dialog(width: f32) -> View {
    column()
        .w(width)
        .p(22.)
        .gap(14.)
        .rounded(16.)
        .bg(Color(28, 28, 34, 250))
        .border(1.)
        .border_color(theme::GLASS_BORDER)
        .shadow(BoxShadow {
            color: Color(0, 0, 0, 170),
            offset: Transform { x: 0., y: 18. },
            blur_radius: 48.,
            spread: 0.,
        })
        .text_color(theme::TEXT)
}

pub fn confirm_delete(app: Rc<App>) -> View {
    let (cancel, del) = (app.clone(), app.clone());
    modal("Delete file", app.confirm_open.clone()).w(380.).p(0.).bg(Color(0, 0, 0, 0)).rounded(16.).overflow_visible().child(
        dialog(380.)
            .child(
                row()
                    .gap(12.)
                    .items_center()
                    .child(
                        div()
                            .size(36., 36.)
                            .rounded(18.)
                            .bg(Color(239, 68, 68, 40))
                            .items_center()
                            .justify_center()
                            .child(icon(Icon::Trash, 18., theme::DANGER)),
                    )
                    .child(text("Move to Recycle Bin?").text_size(16.).font_weight(640)),
            )
            .child(
                text_signal({
                    let app = app.clone();
                    move || app.current().map(|e| e.name()).unwrap_or_default()
                })
                .text_color(theme::TEXT_DIM)
                .text_wrap(true)
                .w_full(),
            )
            .child(
                row()
                    .gap(8.)
                    .pt(6.)
                    .justify_end()
                    .child(text_button("Cancel", false, move || { cancel.confirm_open.set(false); }))
                    .child(
                        div()
                            .h(34.)
                            .px(16.)
                            .rounded(theme::RADIUS_SM)
                            .items_center()
                            .justify_center()
                            .bg(theme::DANGER)
                            .hover(|s| s.bg(Color(248, 90, 90, 255)))
                            .child(text("Delete").font_weight(600))
                            .on_click(move || del.delete_current()),
                    ),
            ),
    )
}

pub fn help(app: Rc<App>) -> View {
    let close = app.clone();
    let rows = [
        ("← → / A D", "Previous / next"),
        ("Home / End", "First / last"),
        ("Wheel / pinch", "Zoom at the pointer"),
        ("Shift + wheel", "Previous / next"),
        ("Double-click", "Fit ↔ 100%"),
        ("+ − / 0 / 1", "Zoom in, out / fit / 100%"),
        ("F / F11 / Enter", "Full screen"),
        ("R / Shift+R", "Rotate right / left"),
        ("Ctrl+S", "Save rotation"),
        ("I", "Details"),
        ("Del", "Move to Recycle Bin"),
        ("S", "Slideshow"),
        ("Space / K", "Play / pause video"),
        ("Shift+← → / J", "Seek video"),
        ("M / L", "Mute / loop"),
        ("Ctrl+C / Ctrl+Shift+C", "Copy picture / path"),
        ("Ctrl+E", "Show in folder"),
        ("Ctrl+O", "Open…"),
        ("Esc", "Back out"),
    ];
    let mut grid = column().gap(7.);
    for (k, v) in rows {
        grid = grid.child(
            row()
                .gap(12.)
                .items_center()
                .child(
                    text(k)
                        .w(170.)
                        .text_size(12.)
                        .font_family(FontFamily::Monospace)
                        .text_color(theme::TEXT),
                )
                .child(text(v).text_size(12.5).text_color(theme::TEXT_DIM)),
        );
    }
    modal("Keyboard shortcuts", app.help_open.clone()).w(460.).p(0.).bg(Color(0, 0, 0, 0)).rounded(16.).overflow_visible().child(
        dialog(460.)
            .child(
                row()
                    .items_center()
                    .gap(10.)
                    .child(icon(Icon::Keyboard, 18., theme::TEXT))
                    .child(text("Keyboard shortcuts").text_size(16.).font_weight(640))
                    .child(div().grow())
                    .child(icon_button(Icon::Close, "Close  (Esc)", move || { close.help_open.set(false); })),
            )
            .child(grid),
    )
}

trait LeftBorder {
    fn border_edges_left(self) -> Self;
}
impl LeftBorder for View {
    fn border_edges_left(self) -> Self {
        self.child(div().absolute().left(0.).top(0.).bottom(0.).w(1.).bg(theme::GLASS_BORDER))
    }
}
