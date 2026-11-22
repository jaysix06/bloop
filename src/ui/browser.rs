//! Bloop's own file browser (no Windows file dialog) and the start screen.
use super::{
    icon_button,
    icons::{Icon, icon},
    text_button,
};
use crate::{
    app::App,
    media::{self, MediaKind},
    theme,
};
use std::{path::PathBuf, rc::Rc, sync::Arc};
use zgx::{
    canvas::GradientStop,
    compose::prelude::*,
    scene::{BoxShadow, Color, Transform},
};

#[derive(Clone, PartialEq)]
struct Item {
    name: String,
    path: PathBuf,
    dir: bool,
    kind: Option<MediaKind>,
}

const MAX_ITEMS: usize = 3000;

fn list(dir: &Option<PathBuf>) -> Vec<Item> {
    let Some(dir) = dir else {
        // "This PC": the drives that exist.
        return (b'A'..=b'Z')
            .map(|c| PathBuf::from(format!("{}:\\", c as char)))
            .filter(|p| p.exists())
            .map(|p| Item { name: p.display().to_string(), path: p, dir: true, kind: None })
            .collect();
    };
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut items: Vec<Item> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.starts_with('$') {
                return None;
            }
            let path = e.path();
            let dir = e.file_type().ok()?.is_dir();
            let kind = media::kind_of(&path);
            (dir || kind.is_some()).then_some(Item { name, path, dir, kind })
        })
        .collect();
    items.sort_by(|a, b| b.dir.cmp(&a.dir).then_with(|| media::natural_cmp(&a.name, &b.name)));
    items.truncate(MAX_ITEMS);
    items
}

fn home_dir() -> Option<PathBuf> {
    let pictures = std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Pictures"));
    pictures.filter(|p| p.is_dir())
}

pub fn browser(app: Rc<App>) -> View {
    let start = app
        .current()
        .and_then(|e| e.path.parent().map(|p| p.to_path_buf()))
        .or_else(|| app.recent.get().first().cloned())
        .or_else(home_dir);
    let dir = app.rt.signal(start.clone());
    let items = app.rt.signal(list(&start));
    let scroll_off = app.rt.signal(0f32);
    let navigate = {
        let (dir, items, scroll_off) = (dir.clone(), items.clone(), scroll_off.clone());
        Rc::new(move |to: Option<PathBuf>| {
            items.set(list(&to));
            dir.set(to);
            scroll_off.set(0.);
        })
    };
    let close = app.clone();
    let backdrop = app.clone();
    overlay()
        .absolute()
        .top(0.)
        .left(0.)
        .right(0.)
        .bottom(0.)
        .bg(Color(0, 0, 0, 150))
        .items_center()
        .justify_center()
        .on_click(move || {
            if !backdrop.entries.with(Vec::is_empty) {
                backdrop.browser_open.set(false);
            }
        })
        .child(
            column()
                .w(760.)
                .h(560.)
                .max_w_percent(92.)
                .max_h_percent(86.)
                .rounded(16.)
                .overflow_hidden()
                .bg(Color(24, 24, 30, 252))
                .border(1.)
                .border_color(theme::GLASS_BORDER)
                .shadow(BoxShadow {
                    color: Color(0, 0, 0, 180),
                    offset: Transform { x: 0., y: 20. },
                    blur_radius: 60.,
                    spread: 0.,
                })
                // Clicks inside the panel don't close it.
                .on_click(|| {})
                .child(
                    row()
                        .h(52.)
                        .w_full()
                        .px(12.)
                        .gap(6.)
                        .items_center()
                        .border_color(theme::GLASS_BORDER)
                        .child(icon_button(Icon::Up, "Up", {
                            let (dir, navigate) = (dir.clone(), navigate.clone());
                            move || {
                                let up = dir.get().and_then(|d| d.parent().map(|p| p.to_path_buf()));
                                navigate(up)
                            }
                        }))
                        .child(
                            text_signal({
                                let dir = dir.clone();
                                move || {
                                    dir.get()
                                        .map(|d| short_path(&d, 78))
                                        .unwrap_or_else(|| "This PC".into())
                                }
                            })
                            .text_size(13.)
                            .font_weight(560)
                            .truncate()
                            .text_wrap(false)
                            .flex_basis(0.)
                            .flex_grow(1.)
                            .min_w(0.),
                        )
                        .child(icon_button(Icon::Close, "Close  (Esc)", move || {
                            close.browser_open.set(false);
                        })),
                )
                .child(div().h(1.).w_full().bg(theme::GLASS_BORDER))
                .child(
                    row()
                        .grow()
                        .min_h(0.)
                        .child(sidebar(app.clone(), navigate.clone()))
                        .child(div().w(1.).h_full().bg(theme::GLASS_BORDER))
                        .child(
                            scroll(scroll_off.clone()).grow().min_w(0.).child(
                                column().p(8.).gap(1.).child(keyed(
                                    {
                                        let items = items.clone();
                                        move || items.with(|v| v.iter().map(|i| i.path.clone()).collect())
                                    },
                                    {
                                        let (items, app, navigate) = (items.clone(), app.clone(), navigate.clone());
                                        move |path, _| {
                                            let item = items
                                                .with(|v| v.iter().find(|i| i.path == path).cloned())
                                                .unwrap();
                                            row_item(app.clone(), item, navigate.clone())
                                        }
                                    },
                                ))
                                .child_when(
                                    {
                                        let items = items.clone();
                                        move || items.with(Vec::is_empty).then_some(())
                                    },
                                    |_, _| {
                                        column()
                                            .p(40.)
                                            .items_center()
                                            .child(text("No folders, photos or videos here").text_color(theme::TEXT_FAINT))
                                    },
                                ),
                            ),
                        ),
                )
                .child(div().h(1.).w_full().bg(theme::GLASS_BORDER))
                .child(
                    row()
                        .h(56.)
                        .px(16.)
                        .gap(8.)
                        .items_center()
                        .child(
                            text_signal({
                                let items = items.clone();
                                move || {
                                    let n = items.with(|v| v.iter().filter(|i| !i.dir).count());
                                    match n {
                                        0 => String::new(),
                                        1 => "1 photo or video".into(),
                                        n => format!("{n} photos and videos"),
                                    }
                                }
                            })
                            .text_size(12.)
                            .text_color(theme::TEXT_FAINT),
                        )
                        .child(div().grow())
                        .child(text_button("Open this folder", true, {
                            let (dir, app) = (dir.clone(), app.clone());
                            move || {
                                if let Some(d) = dir.get() {
                                    app.open_path(&d)
                                }
                            }
                        })),
                ),
        )
}

fn sidebar(app: Rc<App>, navigate: Rc<dyn Fn(Option<PathBuf>)>) -> View {
    let place = |label: String, i: Icon, to: Option<PathBuf>| {
        let navigate = navigate.clone();
        row()
            .h(32.)
            .px(10.)
            .gap(10.)
            .items_center()
            .rounded(theme::RADIUS_SM)
            .hover(|s| s.bg(theme::HOVER))
            .child(icon(i, 16., theme::TEXT_DIM))
            .child(text(label).text_size(12.5).truncate().min_w(0.))
            .on_click(move || navigate(to.clone()))
    };
    let mut col = column().w(190.).p(8.).gap(1.).shrink_0();
    col = col.child(place("This PC".into(), Icon::Drive, None));
    if let Some(u) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        for name in ["Pictures", "Videos", "Downloads", "Desktop"] {
            let p = u.join(name);
            if p.is_dir() {
                col = col.child(place(name.into(), Icon::Folder, Some(p)));
            }
        }
    }
    let recent = app.recent.get();
    if !recent.is_empty() {
        col = col.child(
            text("RECENT")
                .text_size(10.)
                .letter_spacing(0.8)
                .font_weight(640)
                .text_color(theme::TEXT_FAINT)
                .pt(14.)
                .pb(4.)
                .px(10.),
        );
        for r in recent.into_iter().take(6) {
            let name = r
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| r.display().to_string());
            col = col.child(place(name, Icon::Folder, Some(r)));
        }
    }
    col
}

fn row_item(app: Rc<App>, item: Item, navigate: Rc<dyn Fn(Option<PathBuf>)>) -> View {
    let i = if item.path.parent().is_none() {
        Icon::Drive
    } else if item.dir {
        Icon::Folder
    } else if item.kind == Some(MediaKind::Video) {
        Icon::Video
    } else {
        Icon::Image
    };
    let color = if item.dir { theme::CORAL } else { theme::TEXT_DIM };
    let path = item.path.clone();
    let dir = item.dir;
    row()
        .h(34.)
        .px(10.)
        .gap(10.)
        .items_center()
        .rounded(theme::RADIUS_SM)
        .hover(|s| s.bg(theme::HOVER))
        .active(|s| s.bg(theme::PRESS))
        .child(icon(i, 17., color))
        .child(text(item.name).text_size(13.).truncate().min_w(0.))
        .on_click(move || {
            if dir {
                navigate(Some(path.clone()))
            } else {
                app.open_path(&path)
            }
        })
}

/// What shows when Bloop starts with nothing to show.
pub fn empty_state(app: Rc<App>) -> View {
    let stops: Arc<[GradientStop]> = Arc::from(vec![
        GradientStop { offset: 0., color: theme::CORAL },
        GradientStop { offset: 1., color: theme::ACCENT },
    ]);
    let open = app.clone();
    let mut col = column()
        .absolute()
        .w_full()
        .h_full()
        .items_center()
        .justify_center()
        .gap(14.)
        .child(
            div()
                .size(72., 72.)
                .rounded(36.)
                .bg_gradient(0.8, stops)
                .shadow(BoxShadow {
                    color: Color(139, 92, 246, 90),
                    offset: Transform { x: 0., y: 12. },
                    blur_radius: 40.,
                    spread: 0.,
                }),
        )
        .child(text("Bloop").text_size(26.).font_weight(700).pt(6.))
        .child(text("Drop photos or videos anywhere, or browse.").text_color(theme::TEXT_DIM))
        .child(div().h(4.))
        .child(text_button("Browse…", true, move || { open.browser_open.set(true); }));
    let recent = app.recent.get();
    if !recent.is_empty() {
        let mut list = column().gap(2.).pt(18.).w(340.).child(
            text("RECENT")
                .text_size(10.)
                .letter_spacing(0.8)
                .font_weight(640)
                .text_color(theme::TEXT_FAINT)
                .pb(4.)
                .px(10.),
        );
        for dir in recent.into_iter().take(5) {
            let app = app.clone();
            let name = short_path(&dir, 46);
            list = list.child(
                row()
                    .h(32.)
                    .px(10.)
                    .gap(10.)
                    .items_center()
                    .rounded(theme::RADIUS_SM)
                    .hover(|s| s.bg(theme::HOVER))
                    .child(icon(Icon::Folder, 15., theme::CORAL))
                    .child(text(name).text_size(12.5).truncate().min_w(0.).text_color(theme::TEXT_DIM))
                    .on_click(move || app.open_path(&dir)),
            );
        }
        col = col.child(list);
    }
    col.child(
        text("F1 for keyboard shortcuts")
            .text_size(11.5)
            .text_color(theme::TEXT_FAINT)
            .pt(18.),
    )
}

/// `C:\Users\me\…\Trips\Japan`: long paths shortened in the middle, so the
/// drive and the folder you care about both stay readable.
fn short_path(p: &std::path::Path, max: usize) -> String {
    let full = p.display().to_string();
    if full.chars().count() <= max {
        return full;
    }
    let parts: Vec<&str> = full.split('\\').filter(|s| !s.is_empty()).collect();
    let head = parts.first().copied().unwrap_or("");
    let mut len = head.len() + 3;
    let mut tail: Vec<&str> = Vec::new();
    for part in parts.iter().skip(1).rev() {
        if len + part.len() + 1 > max && !tail.is_empty() {
            break;
        }
        len += part.len() + 1;
        tail.push(part);
    }
    tail.reverse();
    format!("{head}\\…\\{}", tail.join("\\"))
}
