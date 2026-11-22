//! The small card that tells you an update is on its way or ready.
use super::{icon_button, icons::Icon, text_button};
use crate::{
    app::{App, UpdateUi},
    theme,
};
use std::{rc::Rc, sync::Arc};
use zgx::{
    canvas::GradientStop,
    compose::prelude::*,
    scene::{BoxShadow, Color, Transform},
};

pub fn update_card(app: Rc<App>) -> View {
    // Zero-height and absolute: an overlay child otherwise stretches over the
    // whole window and would swallow every click on the picture.
    div().absolute().top(0.).left(0.).right(0.).h(0.).overflow_visible().child_when(
        {
            let app = app.clone();
            move || match app.update.get() {
                UpdateUi::Hidden => None,
                // Only the kind of card: progress updates restyle in place.
                UpdateUi::Checking => Some(0),
                UpdateUi::Downloading { .. } => Some(1),
                UpdateUi::Ready { .. } => Some(2),
            }
        },
        move |kind, _| {
            let place = app.clone();
            let body = match kind {
                0 => checking(),
                1 => downloading(app.clone()),
                _ => ready(app.clone()),
            };
            column()
                .absolute()
                .top(0.)
                .left(0.)
                .w(320.)
                .p(16.)
                .gap(10.)
                .rounded(14.)
                .bg(theme::GLASS_STRONG)
                .backdrop_blur(30.)
                .border(1.)
                .border_color(theme::GLASS_BORDER)
                .shadow(BoxShadow {
                    color: Color(0, 0, 0, 150),
                    offset: Transform { x: 0., y: 12. },
                    blur_radius: 36.,
                    spread: 0.,
                })
                .reactive_style(move || {
                    let (vw, _) = place.viewport.get();
                    let right = if place.info_open.get() { theme::INFO_W } else { 0. };
                    Styles::new().translate(vw - right - 320. - 16., theme::TITLEBAR_H + 12.)
                })
                .child(body)
        },
    )
}

fn badge() -> View {
    let stops: Arc<[GradientStop]> = Arc::from(vec![
        GradientStop { offset: 0., color: theme::CORAL },
        GradientStop { offset: 1., color: theme::ACCENT },
    ]);
    div().size(10., 10.).rounded(5.).bg_gradient(0.8, stops).shrink_0()
}

fn checking() -> View {
    row()
        .gap(10.)
        .items_center()
        .child(badge())
        .child(text("Checking for updates…").text_color(theme::TEXT_DIM))
}

fn downloading(app: Rc<App>) -> View {
    let (label, bar) = (app.clone(), app.clone());
    column()
        .gap(10.)
        .child(
            row().gap(10.).items_center().child(badge()).child(
                text_signal(move || match label.update.get() {
                    UpdateUi::Downloading { version, .. } => format!("Downloading Bloop {version}…"),
                    _ => String::new(),
                })
                .font_weight(600),
            ),
        )
        .child(
            div().w_full().h(4.).rounded(2.).bg(Color(255, 255, 255, 30)).child(
                div().absolute().left(0.).top(0.).h(4.).rounded(2.).bg(theme::ACCENT).reactive_style(
                    move || {
                        let p = match bar.update.get() {
                            UpdateUi::Downloading { progress, .. } => progress,
                            _ => 1.,
                        };
                        Styles::new().w(288. * p.clamp(0.02, 1.))
                    },
                ),
            ),
        )
}

fn ready(app: Rc<App>) -> View {
    let (version, notes) = match app.update.get() {
        UpdateUi::Ready { version, notes } => (version, notes),
        _ => Default::default(),
    };
    // The first few lines of the release notes, markdown symbols trimmed.
    let summary: Vec<String> = notes
        .lines()
        .map(|l| l.trim().trim_start_matches(['#', '-', '*', ' ']).trim().to_string())
        .filter(|l| !l.is_empty())
        .take(3)
        .collect();
    let (later, restart, skip, page) = (app.clone(), app.clone(), app.clone(), app.clone());
    let mut col = column()
        .gap(10.)
        .child(
            row()
                .gap(10.)
                .items_center()
                .child(badge())
                .child(text(format!("Bloop {version} is ready")).font_weight(640).grow())
                .child(icon_button(Icon::Close, "Later: installs when you close Bloop", move || {
                    later.update_later()
                })),
        )
        .child(
            text(format!(
                "You have {}. Restart now, or it installs when you close Bloop.",
                crate::update::CURRENT
            ))
            .text_size(12.)
            .text_color(theme::TEXT_DIM)
            .text_wrap(true)
            .w_full(),
        );
    if !summary.is_empty() {
        let mut list = column().gap(3.).pl(2.);
        for line in summary {
            list = list.child(
                text(format!("• {line}"))
                    .text_size(12.)
                    .text_color(theme::TEXT_FAINT)
                    .text_wrap(true)
                    .w_full(),
            );
        }
        col = col.child(list);
    }
    col.child(
        row()
            .gap(8.)
            .pt(4.)
            .items_center()
            .child(
                text("Skip this version")
                    .text_size(12.)
                    .text_color(theme::TEXT_FAINT)
                    .hover(|s| s.text_color(theme::TEXT_DIM))
                    .on_click(move || skip.skip_update()),
            )
            .child(
                text("What's new")
                    .text_size(12.)
                    .text_color(theme::ACCENT)
                    .pl(8.)
                    .on_click(move || page.open_release_page()),
            )
            .child(div().grow())
            .child(text_button("Restart", true, move || restart.restart_to_update())),
    )
}
