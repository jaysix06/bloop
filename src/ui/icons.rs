//! Line icons drawn as tiny SVGs (24×24, round 1.75 strokes), tinted at use.
use std::{cell::RefCell, collections::HashMap, sync::Arc};
use zgx::{compose::prelude::*, scene::Color};

macro_rules! icons {
    ($($name:ident => $body:expr),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
        #[allow(dead_code)]
        pub enum Icon { $($name),* }
        fn body(icon: Icon) -> &'static str {
            match icon { $(Icon::$name => $body),* }
        }
    };
}

icons! {
    ChevronLeft => r#"<path d="M14.5 5.5 8 12l6.5 6.5"/>"#,
    ChevronRight => r#"<path d="M9.5 5.5 16 12l-6.5 6.5"/>"#,
    Minimize => r#"<path d="M6 12h12"/>"#,
    Maximize => r#"<rect x="6" y="6" width="12" height="12" rx="2"/>"#,
    Restore => r#"<rect x="6" y="8.5" width="9.5" height="9.5" rx="1.8"/><path d="M9 6h7.2A1.8 1.8 0 0 1 18 7.8V15"/>"#,
    Close => r#"<path d="M7 7l10 10M17 7 7 17"/>"#,
    Info => r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 11v5.2"/><circle cx="12" cy="7.9" r=".6" fill="white"/>"#,
    RotateRight => r#"<path d="M19 12a7 7 0 1 1-2.05-4.95"/><path d="M19 4.5v4h-4"/>"#,
    RotateLeft => r#"<path d="M5 12a7 7 0 1 0 2.05-4.95"/><path d="M5 4.5v4h4"/>"#,
    Trash => r#"<path d="M5 7h14M10 7V5.2c0-.66.54-1.2 1.2-1.2h1.6c.66 0 1.2.54 1.2 1.2V7M7 7l.8 11.2A2 2 0 0 0 9.8 20h4.4a2 2 0 0 0 2-1.8L17 7"/>"#,
    Copy => r#"<rect x="8.5" y="8.5" width="11" height="11" rx="2"/><path d="M15.5 8.5V6.5a2 2 0 0 0-2-2h-7a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2"/>"#,
    Folder => r#"<path d="M3.5 7.5A2 2 0 0 1 5.5 5.5h3.6l2 2.2h7.4a2 2 0 0 1 2 2v7.8a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2z"/>"#,
    FolderOpen => r#"<path d="M3.5 17.5V7.5a2 2 0 0 1 2-2h3.6l2 2.2H17a2 2 0 0 1 2 2V11"/><path d="M3.5 17.5 6.2 11.6A1.8 1.8 0 0 1 7.8 10.6h12.4a1 1 0 0 1 .9 1.4l-2.4 5.4a2 2 0 0 1-1.8 1.1H5.5a2 2 0 0 1-2-1z"/>"#,
    Play => r#"<path d="M8 5.8v12.4a.8.8 0 0 0 1.2.7l9.6-6.2a.8.8 0 0 0 0-1.4L9.2 5.1A.8.8 0 0 0 8 5.8z" fill="white"/>"#,
    Pause => r#"<rect x="7" y="5.5" width="3.4" height="13" rx="1" fill="white"/><rect x="13.6" y="5.5" width="3.4" height="13" rx="1" fill="white"/>"#,
    Volume => r#"<path d="M4.5 9.5h3l4.5-4v13l-4.5-4h-3z"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/>"#,
    Mute => r#"<path d="M4.5 9.5h3l4.5-4v13l-4.5-4h-3z"/><path d="M16 9.5l5 5M21 9.5l-5 5"/>"#,
    Loop => r#"<path d="M5 11V9.5A2.5 2.5 0 0 1 7.5 7H18l-2.5-2.5M19 13v1.5a2.5 2.5 0 0 1-2.5 2.5H6l2.5 2.5"/>"#,
    Fullscreen => r#"<path d="M4.5 9V5.8c0-.7.6-1.3 1.3-1.3H9M15 4.5h3.2c.7 0 1.3.6 1.3 1.3V9M19.5 15v3.2c0 .7-.6 1.3-1.3 1.3H15M9 19.5H5.8c-.7 0-1.3-.6-1.3-1.3V15"/>"#,
    ExitFullscreen => r#"<path d="M9 4.5v3.2c0 .7-.6 1.3-1.3 1.3H4.5M19.5 9h-3.2c-.7 0-1.3-.6-1.3-1.3V4.5M15 19.5v-3.2c0-.7.6-1.3 1.3-1.3h3.2M4.5 15h3.2c.7 0 1.3.6 1.3 1.3v3.2"/>"#,
    ZoomIn => r#"<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15l4.5 4.5M8 10.5h5M10.5 8v5"/>"#,
    ZoomOut => r#"<circle cx="10.5" cy="10.5" r="6"/><path d="M15 15l4.5 4.5M8 10.5h5"/>"#,
    Slideshow => r#"<rect x="3.5" y="5" width="17" height="12" rx="2"/><path d="M10.5 8.8v4.4l3.6-2.2z" fill="white"/><path d="M8 20h8"/>"#,
    More => r#"<circle cx="6" cy="12" r="1.2" fill="white"/><circle cx="12" cy="12" r="1.2" fill="white"/><circle cx="18" cy="12" r="1.2" fill="white"/>"#,
    Image => r#"<rect x="3.5" y="4.5" width="17" height="15" rx="2.5"/><circle cx="9" cy="9.5" r="1.6"/><path d="M20.5 15.5l-4.6-4.6a1.5 1.5 0 0 0-2.1 0L5 19.5"/>"#,
    Video => r#"<rect x="3.5" y="6" width="12.5" height="12" rx="2.2"/><path d="M16 10.5l4.5-2.6v8.2L16 13.5"/>"#,
    Up => r#"<path d="M12 19V5M6 11l6-6 6 6"/>"#,
    Drive => r#"<rect x="3.5" y="13" width="17" height="6" rx="2"/><path d="M5.5 13l2-7.2A1.5 1.5 0 0 1 9 4.8h6a1.5 1.5 0 0 1 1.5 1l2 7.2"/><circle cx="16.5" cy="16" r=".7" fill="white"/>"#,
    Keyboard => r#"<rect x="3" y="6.5" width="18" height="11" rx="2"/><path d="M7 10h.01M10 10h.01M13 10h.01M16 10h.01M8 14h8"/>"#,
    Ring => r#"<circle cx="12" cy="12" r="9"/>"#,
    Arc => r#"<path d="M12 3a9 9 0 0 1 9 9"/>"#,
    Check =>r#"<path d="M5.5 12.5l4 4 9-9"/>"#,
    Save => r#"<path d="M5 5.5A1.5 1.5 0 0 1 6.5 4h9.4L19 7.1v10.4a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 5 17.5z"/><path d="M8.5 4v4h6V4M8 19v-5h8v5"/>"#,
}

thread_local! {
    static CACHE: RefCell<HashMap<Icon, Arc<SvgData>>> = RefCell::new(HashMap::new());
}

fn data(icon: Icon) -> Arc<SvgData> {
    CACHE.with(|c| {
        c.borrow_mut()
            .entry(icon)
            .or_insert_with(|| {
                let src = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round">{}</svg>"#,
                    body(icon)
                );
                Arc::new(SvgData::new(src.into_bytes()).expect("valid icon svg"))
            })
            .clone()
    })
}

pub fn icon(icon: Icon, size: f32, color: Color) -> View {
    svg(format!("{icon:?}"), data(icon)).size(size, size).svg_tint(color).pointer_events(false)
}
