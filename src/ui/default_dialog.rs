//! "Make Bloop as Default": choose the formats, then finish in Settings.
//!
//! Windows reserves the final say for the user, so this registers Bloop for
//! the ticked formats and opens Settings › Default apps on Bloop's own page,
//! where exactly those formats are listed ready to confirm. While the dialog
//! stays open it follows the choice live.
use super::{icon_button, icons::{Icon, icon}, text_button};
use crate::{app::App, media::FORMATS, theme};
use std::{rc::Rc, sync::Arc};
use zgx::{
    canvas::GradientStop,
    compose::prelude::*,
    scene::{BoxShadow, Color, Transform},
};

const WIDTH: f32 = 540.;
const GREEN: Color = Color(52, 211, 153, 255);

pub fn default_dialog(app: Rc<App>) -> View {
    modal("Make Bloop as Default", app.default_open.clone())
        .w(WIDTH)
        .p(0.)
        .bg(Color(0, 0, 0, 0))
        .rounded(18.)
        .overflow_visible()
        .child(
            column()
                .w(WIDTH)
                .p(24.)
                .gap(18.)
                .rounded(18.)
                .bg(Color(26, 26, 32, 252))
                .border(1.)
                .border_color(theme::GLASS_BORDER)
                .shadow(BoxShadow {
                    color: Color(0, 0, 0, 180),
                    offset: Transform { x: 0., y: 20. },
                    blur_radius: 56.,
                    spread: 0.,
                })
                .text_color(theme::TEXT)
                .child_when(
                    {
                        let app = app.clone();
                        move || Some(app.default_sent.get())
                    },
                    move |sent, _| if sent { finish(app.clone()) } else { choose(app.clone()) },
                ),
        )
}

fn logo() -> View {
    let stops: Arc<[GradientStop]> = Arc::from(vec![
        GradientStop { offset: 0., color: theme::CORAL },
        GradientStop { offset: 1., color: theme::ACCENT },
    ]);
    div().size(36., 36.).rounded(18.).bg_gradient(0.8, stops).shrink_0()
}

fn header(app: &Rc<App>, title: &str, sub: &str) -> View {
    let close = app.clone();
    row()
        .w_full()
        .gap(14.)
        .items_start()
        .child(logo())
        .child(
            column()
                .grow()
                .min_w(0.)
                .gap(4.)
                .child(text(title.to_string()).text_size(17.).font_weight(680))
                .child(
                    text(sub.to_string())
                        .text_size(12.5)
                        .text_color(theme::TEXT_DIM)
                        .text_wrap(true)
                        .w_full(),
                ),
        )
        .child(icon_button(Icon::Close, "Close  (Esc)", move || close.close_default_dialog()))
}

fn choose(app: Rc<App>) -> View {
    let count = app.clone();
    let apply = app.clone();
    let not_now = app.clone();
    let can = app.clone();
    column()
        .w_full()
        .gap(18.)
        .child(header(
            &app,
            "Make Bloop your default viewer",
            "Pick the formats Bloop should open. Windows then shows Bloop's page in Settings with these ready to confirm.",
        ))
        .child(group(app.clone(), "Photos", false))
        .child(group(app.clone(), "Videos", true))
        .child(
            row()
                .w_full()
                .gap(8.)
                .pt(4.)
                .items_center()
                .child(
                    text_signal(move || {
                        let n = count.default_selected.with(|v| v.iter().filter(|x| **x).count());
                        match n {
                            0 => "Nothing selected".into(),
                            1 => "1 format selected".into(),
                            n => format!("{n} formats selected"),
                        }
                    })
                    .text_size(12.)
                    .text_color(theme::TEXT_FAINT)
                    .grow(),
                )
                .child(text_button("Not now", false, move || not_now.close_default_dialog()))
                .child(
                    text_button("Make Bloop as Default", true, move || apply.apply_default())
                        .reactive_style(move || {
                            let any = can.default_selected.with(|v| v.iter().any(|x| *x));
                            Styles::new().opacity(if any { 1. } else { 0.4 })
                        }),
                ),
        )
}

fn group(app: Rc<App>, title: &str, video: bool) -> View {
    let (all, none) = (app.clone(), app.clone());
    let mut chips = row().w_full().gap(8.).flex_wrap();
    for (i, f) in FORMATS.iter().enumerate().filter(|(_, f)| f.video == video) {
        chips = chips.child(chip(app.clone(), i, f.label, f.exts));
    }
    column()
        .w_full()
        .gap(10.)
        .child(
            row()
                .w_full()
                .gap(12.)
                .items_center()
                .child(
                    text(title.to_uppercase())
                        .text_size(10.5)
                        .letter_spacing(0.8)
                        .font_weight(640)
                        .text_color(theme::TEXT_FAINT)
                        .grow(),
                )
                .child(link("All", move || all.set_default_group(video, true)))
                .child(link("None", move || none.set_default_group(video, false))),
        )
        .child(chips)
}

fn link(label: &str, on_click: impl FnMut() + 'static) -> View {
    div()
        .px(4.)
        .rounded(4.)
        .hover(|s| s.bg(theme::HOVER))
        .child(text(label.to_string()).text_size(12.).text_color(theme::ACCENT))
        .on_click(on_click)
}

fn chip(app: Rc<App>, i: usize, label: &str, exts: &[&str]) -> View {
    let (style, mark, toggle) = (app.clone(), app.clone(), app.clone());
    let tip = exts.iter().map(|e| format!(".{e}")).collect::<Vec<_>>().join("  ");
    row()
        .h(32.)
        .px(12.)
        .gap(6.)
        .rounded(16.)
        .items_center()
        .border(1.)
        .reactive_style(move || {
            let on = style.default_selected.with(|v| v[i]);
            Styles::new()
                .bg(if on { theme::ACCENT_SOFT } else { Color(255, 255, 255, 8) })
                .border_color(if on { theme::ACCENT } else { Color(255, 255, 255, 30) })
        })
        .hover(|s| s.opacity(0.9))
        .child(div().child_when(
            move || {
                let on = mark.default_selected.with(|v| v[i]);
                let done = mark.default_status.with(|v| v[i]);
                Some((on, done))
            },
            |(on, done), _| {
                if done {
                    icon(Icon::Check, 14., GREEN)
                } else if on {
                    icon(Icon::Check, 14., theme::TEXT)
                } else {
                    div().size(14., 14.)
                }
            },
        ))
        .child(text(label.to_string()).text_size(12.5).font_weight(560))
        .on_click(move || toggle.toggle_default_format(i))
        .tooltip(if app.default_status.with(|v| v[i]) {
            format!("{tip}\nAlready opens with Bloop")
        } else {
            tip
        })
}

fn finish(app: Rc<App>) -> View {
    let (progress, bar, done) = (app.clone(), app.clone(), app.clone());
    let picked = move |app: &App| -> (usize, usize) {
        let sel = app.default_selected.get();
        let st = app.default_status.get();
        let total = sel.iter().filter(|x| **x).count();
        let ok = sel.iter().zip(&st).filter(|(s, d)| **s && **d).count();
        (ok, total)
    };
    let mut chips = row().w_full().gap(8.).flex_wrap();
    let sel = app.default_selected.get();
    for (i, f) in FORMATS.iter().enumerate().filter(|(i, _)| sel[*i]) {
        let st = app.clone();
        chips = chips.child(
            row()
                .h(28.)
                .px(10.)
                .gap(6.)
                .rounded(14.)
                .items_center()
                .border(1.)
                .reactive_style(move || {
                    let done = st.default_status.with(|v| v[i]);
                    Styles::new()
                        .bg(if done { Color(52, 211, 153, 36) } else { Color(255, 255, 255, 8) })
                        .border_color(if done { GREEN } else { Color(255, 255, 255, 30) })
                })
                .child(div().child_when(
                    {
                        let app = app.clone();
                        move || Some(app.default_status.with(|v| v[i]))
                    },
                    |done, _| {
                        if done { icon(Icon::Check, 13., GREEN) } else { div().size(13., 13.) }
                    },
                ))
                .child(text(f.label).text_size(12.)),
        );
    }
    column()
        .w_full()
        .gap(16.)
        .child(header(
            &app,
            "Finish in Settings",
            "Windows asks you to confirm defaults yourself. Settings is open on Bloop's page.",
        ))
        .child(
            column()
                .w_full()
                .gap(8.)
                .px(4.)
                .child(step(1, "If Bloop's page has a Set default button at the top, click it."))
                .child(step(2, "Otherwise click each format there and choose Bloop."))
                .child(step(3, "Come back here; the list below updates as you go.")),
        )
        .child(
            column()
                .w_full()
                .gap(8.)
                .child(
                    text_signal(move || {
                        let (ok, total) = picked(&progress);
                        if ok == total && total > 0 {
                            "All set: Bloop opens every format you picked.".into()
                        } else {
                            format!("{ok} of {total} formats open with Bloop")
                        }
                    })
                    .text_size(12.5)
                    .font_weight(600),
                )
                .child(
                    div().w_full().h(4.).rounded(2.).bg(Color(255, 255, 255, 24)).child(
                        div().absolute().left(0.).top(0.).h(4.).rounded(2.).reactive_style(move || {
                            let (ok, total) = picked(&bar);
                            let f = if total == 0 { 0. } else { ok as f32 / total as f32 };
                            Styles::new()
                                .w((WIDTH - 48.) * f.max(0.01))
                                .bg(if f >= 1. { GREEN } else { theme::ACCENT })
                        }),
                    ),
                )
                .child(chips),
        )
        .child(
            row()
                .w_full()
                .gap(8.)
                .pt(4.)
                .justify_end()
                .child(text_button("Open Settings again", false, move || {
                    crate::shell::open_default_apps_settings()
                }))
                .child(text_button("Done", true, move || done.close_default_dialog())),
        )
}

fn step(n: u32, s: &str) -> View {
    row()
        .gap(10.)
        .items_center()
        .child(
            div()
                .size(20., 20.)
                .rounded(10.)
                .bg(theme::ACCENT_SOFT)
                .items_center()
                .justify_center()
                .child(text(n.to_string()).text_size(11.).font_weight(700)),
        )
        .child(text(s.to_string()).text_size(12.5).text_color(theme::TEXT_DIM))
}
