//! Bloop's state and everything that changes it. The UI reads signals from
//! here and calls these methods; background work reports back through
//! channels drained by tasks on the UI thread.
use crate::{
    decode::{self, Decoded},
    loader::Loader,
    media::{self, Entry, MediaKind},
    meta::{self, Meta},
    theme,
    video::{VideoCmd, VideoMsg, VideoPlayer, VideoStatus},
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use zgx::{image::ImageData, platform::PlatformService, reactive::Runtime, reactive::Signal};
use zgx_components::{Toast, ToastKind, Toaster};
use zgx_desktop::{TaskSpawner, WindowHandle};

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ViewT {
    pub scale: f32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Status {
    Empty,
    Loading,
    Ready,
    Error(String),
}

pub const MIN_ZOOM: f32 = 0.02;
pub const MAX_ZOOM: f32 = 40.;

pub struct App {
    pub rt: Runtime,
    pub window: WindowHandle,
    pub tasks: TaskSpawner,
    pub viewport: Signal<(f32, f32)>,
    pub entries: Signal<Vec<Entry>>,
    pub index: Signal<usize>,
    pub status: Signal<Status>,
    pub kind: Signal<Option<MediaKind>>,
    pub image: Signal<Arc<ImageData>>,
    pub size: Signal<(f32, f32)>,
    pub prev_image: Signal<Arc<ImageData>>,
    pub prev_size: Signal<(f32, f32)>,
    pub fade: Signal<f32>,
    pub view: Signal<ViewT>,
    target: Cell<ViewT>,
    pub fit: Signal<bool>,
    animating: Cell<bool>,
    fading: Cell<u64>,
    rotation: Cell<i32>,
    rotations: RefCell<HashMap<PathBuf, i32>>,
    original: RefCell<Option<Arc<Decoded>>>,
    pub chrome: Signal<f32>,
    chrome_target: Cell<f32>,
    chrome_animating: Cell<bool>,
    last_activity: Cell<Instant>,
    pub chrome_hover: Cell<bool>,
    pub info_open: Signal<bool>,
    pub confirm_open: Signal<bool>,
    pub browser_open: Signal<bool>,
    pub help_open: Signal<bool>,
    pub fullscreen: Signal<bool>,
    pub maximized: Signal<bool>,
    pub slideshow: Signal<bool>,
    slideshow_epoch: Cell<u64>,
    video: RefCell<Option<VideoPlayer>>,
    pub video_status: Signal<VideoStatus>,
    video_gen: Cell<u64>,
    video_tx: async_channel::Sender<VideoMsg>,
    pub volume: Signal<f64>,
    pub muted: Signal<bool>,
    pub looping: Signal<bool>,
    pub loader: Loader,
    thumbs: RefCell<HashMap<PathBuf, Signal<Option<Arc<ImageData>>>>>,
    pub toaster: Rc<Toaster>,
    pub meta: Signal<Option<Meta>>,
    pub drag: Cell<Option<(f32, f32, ViewT)>>,
    pub platform: RefCell<Option<Rc<PlatformService>>>,
    pub strip_offset: Signal<f32>,
    pub edge_flash: Signal<f32>,
    pub drop_hover: Signal<bool>,
    pub registered: Signal<bool>,
    /// A deleted item waiting for its Undo window to pass.
    pending_delete: RefCell<Option<(u64, Entry)>>,
    delete_epoch: Cell<u64>,
    pub recent: Signal<Vec<PathBuf>>,
    pub update: Signal<UpdateUi>,
    update_ready: RefCell<Option<(crate::update::Release, PathBuf)>>,
    install_on_exit: Cell<bool>,
}

/// The update card's state.
#[derive(Clone, PartialEq, Debug)]
pub enum UpdateUi {
    Hidden,
    Checking,
    Downloading { version: String, progress: f32 },
    Ready { version: String, notes: String },
}

fn blank() -> Arc<ImageData> {
    Arc::new(ImageData::new(1, 1, vec![0, 0, 0, 0]).unwrap())
}

impl App {
    pub fn new(
        rt: Runtime,
        window: WindowHandle,
        tasks: TaskSpawner,
        viewport: Signal<(f32, f32)>,
    ) -> Rc<Self> {
        let (video_tx, video_rx) = async_channel::unbounded();
        let toaster = Toaster::of(&rt);
        let identity = ViewT { scale: 1., x: 0., y: 0. };
        let app = Rc::new(Self {
            window,
            tasks,
            viewport,
            entries: rt.signal(Vec::new()),
            index: rt.signal(0),
            status: rt.signal(Status::Empty),
            kind: rt.signal(None),
            image: rt.signal(blank()),
            size: rt.signal((1., 1.)),
            prev_image: rt.signal(blank()),
            prev_size: rt.signal((1., 1.)),
            fade: rt.signal(1.),
            view: rt.signal(identity),
            target: Cell::new(identity),
            fit: rt.signal(true),
            animating: Cell::new(false),
            fading: Cell::new(0),
            rotation: Cell::new(0),
            rotations: Default::default(),
            original: Default::default(),
            chrome: rt.signal(1.),
            chrome_target: Cell::new(1.),
            chrome_animating: Cell::new(false),
            last_activity: Cell::new(Instant::now()),
            chrome_hover: Cell::new(false),
            info_open: rt.signal(false),
            confirm_open: rt.signal(false),
            browser_open: rt.signal(false),
            help_open: rt.signal(false),
            fullscreen: rt.signal(false),
            maximized: rt.signal(false),
            slideshow: rt.signal(false),
            slideshow_epoch: Cell::new(0),
            video: Default::default(),
            video_status: rt.signal(VideoStatus::default()),
            video_gen: Cell::new(0),
            video_tx,
            volume: rt.signal(0.8),
            muted: rt.signal(false),
            looping: rt.signal(true),
            loader: Loader::new(),
            thumbs: Default::default(),
            toaster,
            meta: rt.signal(None),
            drag: Cell::new(None),
            platform: Default::default(),
            strip_offset: rt.signal(0.),
            edge_flash: rt.signal(0.),
            drop_hover: rt.signal(false),
            registered: rt.signal(crate::shell::is_registered()),
            pending_delete: Default::default(),
            delete_epoch: Cell::new(0),
            recent: rt.signal(crate::recent::load()),
            update: rt.signal(UpdateUi::Hidden),
            update_ready: Default::default(),
            install_on_exit: Cell::new(false),
            rt,
        });
        app.start_pumps(video_rx);
        // The daily automatic check, a few seconds after startup so it never
        // competes with opening the first picture. Only an installed copy
        // updates itself.
        if crate::update::is_configured() && crate::update::is_installed_copy() && crate::update::due() {
            let weak = Rc::downgrade(&app);
            app.tasks.spawn(async move {
                zgx::timer::sleep(Duration::from_secs(5)).await;
                if let Some(app) = weak.upgrade() {
                    app.check_updates(false);
                }
            });
        }
        app
    }

    // ----- background results ------------------------------------------------

    fn start_pumps(self: &Rc<Self>, video_rx: async_channel::Receiver<VideoMsg>) {
        let weak = Rc::downgrade(self);
        let results = self.loader.results.clone();
        self.tasks.spawn(async move {
            while let Ok(loaded) = results.recv().await {
                let Some(app) = weak.upgrade() else { break };
                app.loader.accept(&loaded);
                let is_current = app.current().is_some_and(|e| e.path == loaded.path);
                if is_current && app.status.get() == Status::Loading {
                    match loaded.result {
                        Ok(decoded) => app.present(decoded, false),
                        Err(e) if e != "skipped" => { app.status.set(Status::Error(e)); }
                        Err(_) => app.request_current(),
                    }
                }
            }
        });
        let weak = Rc::downgrade(self);
        let thumbs = self.loader.thumbs.clone();
        self.tasks.spawn(async move {
            while let Ok(thumb) = thumbs.recv().await {
                let Some(app) = weak.upgrade() else { break };
                if let Some(sig) = app.thumbs.borrow().get(&thumb.path) {
                    sig.set(thumb.image.clone());
                }
                // A video's poster until its first frame arrives.
                let poster = app.current().is_some_and(|e| {
                    e.path == thumb.path && e.kind == MediaKind::Video
                }) && app.video_status.get().width == 0
                    && app.status.get() == Status::Loading;
                if poster && let Some(img) = thumb.image {
                    app.size.set((img.width() as f32, img.height() as f32));
                    app.image.set(img);
                    app.status.set(Status::Ready);
                }
            }
        });
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            while let Ok(msg) = video_rx.recv().await {
                let Some(app) = weak.upgrade() else { break };
                match msg {
                    VideoMsg::Frame(g, frame) if g == app.video_gen.get() => {
                        let size = (frame.width() as f32, frame.height() as f32);
                        if app.size.get() != size {
                            app.size.set(size);
                        }
                        app.image.set(frame);
                        if app.status.get() != Status::Ready {
                            app.status.set(Status::Ready);
                        }
                    }
                    VideoMsg::Status(g, s) if g == app.video_gen.get() => {
                        if app.video_status.get() != s {
                            app.video_status.set(s);
                        }
                    }
                    VideoMsg::Error(g, e) if g == app.video_gen.get() => {
                        app.status.set(Status::Error(e));
                    }
                    _ => {}
                }
            }
        });
        // Chrome auto-hide.
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            loop {
                zgx::timer::sleep(Duration::from_millis(250)).await;
                let Some(app) = weak.upgrade() else { break };
                let keep = app.chrome_hover.get()
                    || app.info_open.get()
                    || app.browser_open.get()
                    || app.confirm_open.get()
                    || app.help_open.get()
                    || matches!(app.status.get(), Status::Empty | Status::Error(_))
                    || (app.kind.get() == Some(MediaKind::Video) && !app.video_status.get().playing);
                if !keep && app.last_activity.get().elapsed() > theme::CHROME_IDLE {
                    app.set_chrome(0.);
                }
            }
        });
    }

    // ----- navigation ----------------------------------------------------------

    pub fn current(&self) -> Option<Entry> {
        self.entries.with(|e| e.get(self.index.get()).cloned())
    }

    pub fn open_path(self: &Rc<Self>, path: &Path) {
        let (entries, index) = media::open_target(path);
        if entries.is_empty() {
            self.toast_error("Nothing to show", "That folder has no photos or videos.");
            return;
        }
        if let Some(dir) = entries[index].path.parent() {
            self.recent.set(crate::recent::push(dir));
        }
        self.browser_open.set(false);
        self.thumbs.borrow_mut().clear();
        self.entries.set(entries);
        self.go(index, true);
    }

    pub fn step(self: &Rc<Self>, delta: isize) {
        let n = self.entries.with(Vec::len);
        if n == 0 {
            return;
        }
        let i = self.index.get() as isize + delta;
        let wrapped = i.rem_euclid(n as isize) as usize;
        if i < 0 || i >= n as isize {
            self.flash_edge();
        }
        self.go(wrapped, false);
    }

    pub fn first(self: &Rc<Self>) {
        self.go(0, false);
    }
    pub fn last(self: &Rc<Self>) {
        let n = self.entries.with(Vec::len);
        if n > 0 {
            self.go(n - 1, false);
        }
    }

    pub fn go(self: &Rc<Self>, index: usize, initial: bool) {
        let Some(entry) = self.entries.with(|e| e.get(index).cloned()) else {
            self.status.set(Status::Empty);
            return;
        };
        let same = !initial && index == self.index.get() && self.status.get() == Status::Ready;
        if same {
            return;
        }
        // Leave the old item: stop its video, remember its picture to fade out.
        self.video.borrow_mut().take();
        self.video_gen.set(self.video_gen.get() + 1);
        self.video_status.set(VideoStatus::default());
        if self.status.get() == Status::Ready {
            self.prev_image.set(self.image.get());
            self.prev_size.set(self.fitted_size());
        }
        self.index.set(index);
        self.kind.set(Some(entry.kind));
        self.rotation.set(self.rotations.borrow().get(&entry.path).copied().unwrap_or(0));
        self.meta.set(None);
        self.reset_view();
        self.preload();
        if entry.kind == MediaKind::Video {
            self.status.set(Status::Loading);
            self.image.set(blank());
            self.size.set((16., 9.));
            self.loader.thumbnail(entry.path.clone());
            let player = VideoPlayer::open(
                entry.path.clone(),
                self.video_gen.get(),
                self.video_tx.clone(),
                if self.muted.get() { 0. } else { self.volume.get() },
                self.looping.get(),
            );
            *self.video.borrow_mut() = Some(player);
            self.start_fade();
        } else if let Some(decoded) = self.loader.cached(&entry.path) {
            self.present(decoded, true);
        } else {
            self.status.set(Status::Loading);
            self.start_fade();
        }
        if self.info_open.get() {
            self.load_meta();
        }
        self.scroll_strip_to_current();
        self.restart_slideshow_timer();
    }

    fn request_current(&self) {
        self.preload();
    }

    /// Current first, then the neighbours either side.
    fn preload(&self) {
        let entries = self.entries.get();
        let n = entries.len();
        if n == 0 {
            return;
        }
        let i = self.index.get();
        let mut want = vec![entries[i].clone()];
        for d in [1isize, -1, 2, -2] {
            let j = (i as isize + d).rem_euclid(n as isize) as usize;
            if !want.iter().any(|e| e.path == entries[j].path) {
                want.push(entries[j].clone());
            }
        }
        self.loader.want(&want);
    }

    fn present(self: &Rc<Self>, decoded: Arc<Decoded>, immediate: bool) {
        *self.original.borrow_mut() = Some(decoded.clone());
        let turns = self.rotation.get();
        let image = if turns != 0 && decoded.frames == 1 {
            decode::rotate(&decoded.image, turns).unwrap_or(decoded.image.clone())
        } else {
            decoded.image.clone()
        };
        self.size.set((image.width() as f32, image.height() as f32));
        self.image.set(image);
        self.status.set(Status::Ready);
        self.reset_view();
        if !immediate || self.fade.get() >= 1. {
            self.start_fade();
        }
    }

    fn start_fade(self: &Rc<Self>) {
        let epoch = self.fading.get() + 1;
        self.fading.set(epoch);
        self.fade.set(0.);
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            let start = Instant::now();
            loop {
                let Some(app) = weak.upgrade() else { break };
                if app.fading.get() != epoch {
                    break;
                }
                let t = (start.elapsed().as_secs_f32() * 1000. / theme::FADE_MS).min(1.);
                app.fade.set(1. - (1. - t).powi(3));
                if t >= 1. {
                    break;
                }
                drop(app);
                zgx::timer::sleep(Duration::from_millis(8)).await;
            }
        });
    }

    fn flash_edge(self: &Rc<Self>) {
        self.edge_flash.set(1.);
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            for k in (0..12).rev() {
                zgx::timer::sleep(Duration::from_millis(16)).await;
                let Some(app) = weak.upgrade() else { break };
                app.edge_flash.set(k as f32 / 12.);
            }
        });
    }

    // ----- zoom & pan ------------------------------------------------------------

    pub fn fit_scale(&self) -> f32 {
        let (vw, vh) = self.viewport.get();
        let (iw, ih) = self.size.get();
        (vw / iw).min(vh / ih).min(1.).max(MIN_ZOOM)
    }

    fn fitted_size(&self) -> (f32, f32) {
        let v = self.view.get();
        let (iw, ih) = self.size.get();
        (iw * v.scale, ih * v.scale)
    }

    pub fn reset_view(self: &Rc<Self>) {
        self.fit.set(true);
        let v = ViewT { scale: self.fit_scale(), x: 0., y: 0. };
        self.target.set(v);
        self.view.set(v);
    }

    /// Re-fit after the window or picture changes size.
    pub fn refit(self: &Rc<Self>) {
        if self.fit.get() {
            self.reset_view();
        } else {
            let t = self.clamp(self.target.get());
            self.target.set(t);
            self.view.set(self.clamp(self.view.get()));
        }
    }

    /// `refit` from an effect, without subscribing it to the view.
    pub fn refit_untracked(self: &Rc<Self>) {
        let rt = self.rt.clone();
        rt.untracked(|| self.refit());
    }

    fn clamp(&self, v: ViewT) -> ViewT {
        let (vw, vh) = self.viewport.get();
        let (iw, ih) = self.size.get();
        let (w, h) = (iw * v.scale, ih * v.scale);
        let mx = ((w - vw) / 2.).max(0.);
        let my = ((h - vh) / 2.).max(0.);
        ViewT { scale: v.scale, x: v.x.clamp(-mx, mx), y: v.y.clamp(-my, my) }
    }

    /// Zoom by `factor` keeping the point under (`px`, `py`) still.
    pub fn zoom_at(self: &Rc<Self>, factor: f32, px: f32, py: f32) {
        if self.status.get() != Status::Ready {
            return;
        }
        let t = self.target.get();
        let fit = self.fit_scale();
        let mut s = (t.scale * factor).clamp(MIN_ZOOM.min(fit), MAX_ZOOM);
        // Snap to fit when crossing it, so zooming out lands cleanly.
        if (t.scale > fit && s < fit) || (t.scale < fit && s > fit) {
            s = fit;
        }
        let (vw, vh) = self.viewport.get();
        let (cx, cy) = (px - vw / 2., py - vh / 2.);
        let k = s / t.scale;
        let next = ViewT { scale: s, x: cx - (cx - t.x) * k, y: cy - (cy - t.y) * k };
        self.fit.set((s - fit).abs() < 1e-4);
        self.animate_to(self.clamp(next));
    }

    pub fn zoom_center(self: &Rc<Self>, factor: f32) {
        let (vw, vh) = self.viewport.get();
        self.zoom_at(factor, vw / 2., vh / 2.);
    }

    pub fn zoom_actual(self: &Rc<Self>, px: f32, py: f32) {
        let t = self.target.get();
        self.zoom_at(1. / t.scale, px, py);
    }

    /// Double-click: fit ↔ 100% (or 2× fit for small pictures) at the pointer.
    pub fn toggle_zoom(self: &Rc<Self>, px: f32, py: f32) {
        if self.fit.get() {
            let fit = self.fit_scale();
            let target = if fit < 0.999 { 1. } else { 2. * fit };
            self.zoom_at(target / self.target.get().scale, px, py);
        } else {
            self.fit.set(true);
            self.animate_to(ViewT { scale: self.fit_scale(), x: 0., y: 0. });
        }
    }

    pub fn pan_to(&self, x: f32, y: f32, from: ViewT) {
        let v = self.clamp(ViewT { scale: from.scale, x, y });
        self.target.set(v);
        self.view.set(v);
    }

    pub fn pan_by(self: &Rc<Self>, dx: f32, dy: f32) {
        let t = self.target.get();
        self.animate_to(self.clamp(ViewT { scale: t.scale, x: t.x + dx, y: t.y + dy }));
    }

    pub fn is_zoomed(&self) -> bool {
        !self.fit.get() && self.target.get().scale > self.fit_scale() + 1e-4
    }

    fn animate_to(self: &Rc<Self>, target: ViewT) {
        self.target.set(target);
        if self.animating.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            let mut last = Instant::now();
            loop {
                zgx::timer::sleep(Duration::from_millis(7)).await;
                let Some(app) = weak.upgrade() else { break };
                let dt = last.elapsed().as_secs_f32().min(0.05);
                last = Instant::now();
                let (v, t) = (app.view.get(), app.target.get());
                let k = 1. - (-dt * 22.).exp();
                // Interpolate scale in log space so zooming feels even.
                let ls = v.scale.ln() + (t.scale.ln() - v.scale.ln()) * k;
                let next = ViewT { scale: ls.exp(), x: v.x + (t.x - v.x) * k, y: v.y + (t.y - v.y) * k };
                let done = (next.scale / t.scale - 1.).abs() < 1e-3
                    && (next.x - t.x).abs() < 0.3
                    && (next.y - t.y).abs() < 0.3;
                app.view.set(if done { t } else { next });
                if done {
                    app.animating.set(false);
                    break;
                }
            }
        });
    }

    // ----- chrome ----------------------------------------------------------------

    pub fn activity(self: &Rc<Self>) {
        self.last_activity.set(Instant::now());
        self.set_chrome(1.);
    }

    fn set_chrome(self: &Rc<Self>, target: f32) {
        if self.chrome_target.get() == target {
            return;
        }
        self.chrome_target.set(target);
        if self.chrome_animating.replace(true) {
            return;
        }
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            loop {
                zgx::timer::sleep(Duration::from_millis(8)).await;
                let Some(app) = weak.upgrade() else { break };
                let (c, t) = (app.chrome.get(), app.chrome_target.get());
                let step = 8. / theme::FADE_MS * if t > c { 1.4 } else { 0.8 };
                let next = if t > c { (c + step).min(t) } else { (c - step).max(t) };
                app.chrome.set(next);
                if next == t {
                    app.chrome_animating.set(false);
                    break;
                }
            }
        });
    }

    // ----- window ----------------------------------------------------------------

    pub fn toggle_fullscreen(&self) {
        let f = !self.fullscreen.get();
        self.fullscreen.set(f);
        self.window.set_fullscreen(f);
    }

    pub fn toggle_maximize(&self) {
        let m = !self.maximized.get();
        self.maximized.set(m);
        self.window.set_maximized(m);
    }

    /// Esc: the topmost thing closes first, then fullscreen, then nothing.
    pub fn escape(self: &Rc<Self>) -> bool {
        if self.confirm_open.get() {
            self.confirm_open.set(false);
        } else if self.help_open.get() {
            self.help_open.set(false);
        } else if self.browser_open.get() && !self.entries.with(Vec::is_empty) {
            self.browser_open.set(false);
        } else if self.info_open.get() {
            self.info_open.set(false);
        } else if self.slideshow.get() {
            self.toggle_slideshow();
        } else if self.is_zoomed() {
            self.toggle_zoom(0., 0.);
        } else if self.fullscreen.get() {
            self.toggle_fullscreen();
        } else {
            return false;
        }
        true
    }

    // ----- video -----------------------------------------------------------------

    pub fn video(&self, cmd: VideoCmd) {
        if let Some(p) = self.video.borrow().as_ref() {
            p.send(cmd);
        }
    }

    pub fn toggle_play(&self) {
        self.video(VideoCmd::Toggle);
        let mut s = self.video_status.get();
        s.playing = !s.playing;
        self.video_status.set(s);
    }

    pub fn seek_by(&self, secs: f64) {
        let s = self.video_status.get();
        self.seek_to((s.position + secs).clamp(0., s.duration.max(0.)));
    }

    pub fn seek_to(&self, t: f64) {
        self.video(VideoCmd::Seek(t));
        let mut s = self.video_status.get();
        s.position = t;
        self.video_status.set(s);
    }

    pub fn set_volume(&self, v: f64) {
        self.volume.set(v.clamp(0., 1.));
        self.muted.set(false);
        self.video(VideoCmd::Volume(v.clamp(0., 1.)));
    }

    pub fn toggle_mute(&self) {
        let m = !self.muted.get();
        self.muted.set(m);
        self.video(VideoCmd::Volume(if m { 0. } else { self.volume.get() }));
    }

    pub fn toggle_loop(&self) {
        let l = !self.looping.get();
        self.looping.set(l);
        self.video(VideoCmd::Loop(l));
    }

    // ----- slideshow -------------------------------------------------------------

    pub fn toggle_slideshow(self: &Rc<Self>) {
        let on = !self.slideshow.get();
        self.slideshow.set(on);
        if on {
            if !self.fullscreen.get() {
                self.toggle_fullscreen();
            }
            self.toast("Slideshow", "Space pauses · Esc stops");
        }
        self.restart_slideshow_timer();
    }

    fn restart_slideshow_timer(self: &Rc<Self>) {
        let epoch = self.slideshow_epoch.get() + 1;
        self.slideshow_epoch.set(epoch);
        if !self.slideshow.get() {
            return;
        }
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            loop {
                zgx::timer::sleep(theme::SLIDESHOW).await;
                let Some(app) = weak.upgrade() else { break };
                if app.slideshow_epoch.get() != epoch || !app.slideshow.get() {
                    break;
                }
                // Let videos play out once before moving on.
                let s = app.video_status.get();
                if app.kind.get() == Some(MediaKind::Video) && s.playing && !s.ended {
                    continue;
                }
                app.step(1);
                break;
            }
        });
    }

    // ----- thumbnails & filmstrip -----------------------------------------------

    pub fn thumb(&self, path: &PathBuf) -> Signal<Option<Arc<ImageData>>> {
        if let Some(s) = self.thumbs.borrow().get(path) {
            return s.clone();
        }
        let s = self.rt.signal(None);
        self.thumbs.borrow_mut().insert(path.clone(), s.clone());
        self.loader.thumbnail(path.clone());
        s
    }

    pub fn scroll_strip_to_current(&self) {
        let (vw, _) = self.viewport.get();
        let n = self.entries.with(Vec::len) as f32;
        let item = theme::THUMB + 6.;
        let content = n * item;
        let center = self.index.get() as f32 * item + item / 2.;
        let max = (content - vw + 32.).max(0.);
        self.strip_offset.set((center - vw / 2.).clamp(0., max));
    }

    // ----- info ------------------------------------------------------------------

    pub fn toggle_info(self: &Rc<Self>) {
        let open = !self.info_open.get();
        self.info_open.set(open);
        if open {
            self.load_meta();
        }
    }

    fn load_meta(&self) {
        let Some(entry) = self.current() else { return };
        let dims = self.original.borrow().as_ref().map(|d| (d.width, d.height));
        let s = self.video_status.get();
        let dims = if entry.kind == MediaKind::Video {
            (s.width > 0).then_some((s.width, s.height))
        } else {
            dims
        };
        let duration = (entry.kind == MediaKind::Video).then_some(s.duration);
        self.meta.set(Some(meta::read(&entry, dims, duration)));
    }

    // ----- actions ---------------------------------------------------------------

    pub fn rotate(self: &Rc<Self>, delta: i32) {
        let Some(entry) = self.current() else { return };
        if entry.kind != MediaKind::Image || self.status.get() != Status::Ready {
            return;
        }
        let turns = (self.rotation.get() + delta).rem_euclid(4);
        self.rotation.set(turns);
        self.rotations.borrow_mut().insert(entry.path.clone(), turns);
        if let Some(orig) = self.original.borrow().clone() {
            let img = decode::rotate(&orig.image, turns).unwrap_or(orig.image.clone());
            self.size.set((img.width() as f32, img.height() as f32));
            self.image.set(img);
            self.reset_view();
        }
    }

    pub fn has_unsaved_rotation(&self) -> bool {
        self.rotation.get() != 0
    }

    pub fn save_rotation(self: &Rc<Self>) {
        let Some(entry) = self.current() else { return };
        let turns = self.rotation.get();
        if turns == 0 {
            return;
        }
        match crate::orient::save_rotation(&entry.path, turns) {
            Ok(how) => {
                self.rotations.borrow_mut().remove(&entry.path);
                self.loader.forget(&entry.path);
                self.rotation.set(0);
                // What's on screen is already rotated: keep it as the new original.
                if let Some(orig) = self.original.borrow_mut().as_mut() {
                    let img = self.image.get();
                    *orig = Arc::new(Decoded {
                        width: img.width(),
                        height: img.height(),
                        image: img,
                        frames: 1,
                    });
                }
                let body = match how {
                    crate::orient::Saved::Lossless => "Saved losslessly.",
                    crate::orient::Saved::Reencoded => "Saved.",
                };
                self.toast("Rotation saved", body);
            }
            Err(e) => self.toast_error("Couldn't save rotation", &e),
        }
    }

    pub fn ask_delete(&self) {
        if self.current().is_some() {
            self.confirm_open.set(true);
        }
    }

    /// Hide the item now; it goes to the Recycle Bin when the Undo toast
    /// expires (or when Bloop closes).
    pub fn delete_current(self: &Rc<Self>) {
        self.confirm_open.set(false);
        let Some(entry) = self.current() else { return };
        self.commit_pending_delete();
        let index = self.index.get();
        self.video.borrow_mut().take();
        let mut entries = self.entries.get();
        entries.remove(index);
        let epoch = self.delete_epoch.get() + 1;
        self.delete_epoch.set(epoch);
        *self.pending_delete.borrow_mut() = Some((epoch, entry.clone()));
        self.loader.forget(&entry.path);
        self.status.set(Status::Loading);
        let n = entries.len();
        self.entries.set(entries);
        if n == 0 {
            self.status.set(Status::Empty);
        } else {
            self.go(index.min(n - 1), true);
        }
        let weak = Rc::downgrade(self);
        let undo_entry = entry.clone();
        self.toaster.show(
            Toast::new("Moved to Recycle Bin")
                .body(entry.name())
                .duration(Duration::from_secs(6))
                .action("Undo", move || {
                    if let Some(app) = weak.upgrade() {
                        app.undo_delete(epoch, index, undo_entry.clone());
                    }
                }),
        );
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            zgx::timer::sleep(Duration::from_millis(6500)).await;
            if let Some(app) = weak.upgrade()
                && app.pending_delete.borrow().as_ref().is_some_and(|(e, _)| *e == epoch)
            {
                app.commit_pending_delete();
            }
        });
    }

    fn undo_delete(self: &Rc<Self>, epoch: u64, index: usize, entry: Entry) {
        let mine = self.pending_delete.borrow().as_ref().is_some_and(|(e, _)| *e == epoch);
        if !mine {
            return;
        }
        self.pending_delete.borrow_mut().take();
        let mut entries = self.entries.get();
        let at = index.min(entries.len());
        entries.insert(at, entry);
        self.entries.set(entries);
        self.go(at, true);
    }

    pub fn commit_pending_delete(&self) {
        if let Some((_, entry)) = self.pending_delete.borrow_mut().take()
            && let Err(e) = crate::shell::recycle(&entry.path)
        {
            self.toast_error("Couldn't delete", &e);
        }
    }

    pub fn copy_image(&self) {
        let Some(platform) = self.platform.borrow().clone() else { return };
        if self.status.get() != Status::Ready {
            return;
        }
        let img = self.image.get();
        // Animated pictures copy their current still.
        let Ok(still) = ImageData::new(img.width(), img.height(), img.pixels().to_vec()) else {
            return;
        };
        let _ = platform.0.set_clipboard_image(Arc::new(still));
        self.toast("Copied", "Picture copied to the clipboard.");
    }

    pub fn copy_path(&self) {
        let Some(platform) = self.platform.borrow().clone() else { return };
        if let Some(e) = self.current() {
            platform.0.set_clipboard_text(e.path.display().to_string());
            self.toast("Copied", "File path copied to the clipboard.");
        }
    }

    pub fn reveal(&self) {
        if let Some(e) = self.current() {
            crate::shell::reveal(&e.path);
        }
    }

    pub fn register(&self) {
        match crate::shell::register() {
            Ok(()) => {
                self.registered.set(true);
                crate::shell::open_default_apps_settings();
                self.toast("Bloop is registered", "Pick Bloop as the default in Settings.");
            }
            Err(e) => self.toast_error("Couldn't register", &e.to_string()),
        }
    }

    // ----- updates -----------------------------------------------------------------

    /// Check GitHub for a newer release and download it in the background.
    /// `manual` (from the menu) reports "up to date" and errors; the daily
    /// automatic check stays silent unless there is an update.
    pub fn check_updates(self: &Rc<Self>, manual: bool) {
        if !crate::update::is_configured() {
            if manual {
                self.toast_error("Updates aren't set up", "This build has no GitHub repository configured.");
            }
            return;
        }
        if matches!(self.update.get(), UpdateUi::Checking | UpdateUi::Downloading { .. }) {
            return;
        }
        if let UpdateUi::Ready { .. } = self.update.get() {
            return;
        }
        if manual {
            self.update.set(UpdateUi::Checking);
        }
        let (tx, rx) = async_channel::unbounded();
        std::thread::Builder::new()
            .name("bloop-update".into())
            .spawn(move || {
                crate::update::check_and_download(manual, |e| {
                    let _ = tx.send_blocking(e);
                })
            })
            .expect("spawn updater");
        let weak = Rc::downgrade(self);
        self.tasks.spawn(async move {
            use crate::update::Event;
            let mut version = String::new();
            while let Ok(event) = rx.recv().await {
                let Some(app) = weak.upgrade() else { break };
                match event {
                    Event::UpToDate => {
                        app.update.set(UpdateUi::Hidden);
                        if manual {
                            app.toast("You're up to date", &format!("Bloop {} is the latest version.", crate::update::CURRENT));
                        }
                    }
                    Event::Found(r) => {
                        version = r.version.clone();
                        app.update.set(UpdateUi::Downloading { version: version.clone(), progress: 0. });
                    }
                    Event::Progress(p) => {
                        if !version.is_empty() {
                            app.update.set(UpdateUi::Downloading { version: version.clone(), progress: p });
                        }
                    }
                    Event::Ready(r, path) => {
                        app.update.set(UpdateUi::Ready { version: r.version.clone(), notes: r.notes.clone() });
                        *app.update_ready.borrow_mut() = Some((r, path));
                        // Unless told otherwise, it goes in when Bloop closes.
                        app.install_on_exit.set(true);
                    }
                    Event::Failed(e) => {
                        app.update.set(UpdateUi::Hidden);
                        if manual {
                            app.toast_error("Couldn't check for updates", &e);
                        }
                    }
                }
            }
        });
    }

    /// Install the downloaded update now and reopen Bloop afterwards.
    pub fn restart_to_update(&self) {
        let Some((_, path)) = self.update_ready.borrow().clone() else { return };
        self.commit_pending_delete();
        match crate::update::launch_installer(&path, true) {
            Ok(()) => {
                self.install_on_exit.set(false);
                self.window.close();
            }
            Err(e) => self.toast_error("Couldn't start the update", &e),
        }
    }

    /// Hide the card; the update still installs when Bloop closes.
    pub fn update_later(&self) {
        self.update.set(UpdateUi::Hidden);
    }

    /// Don't offer this version again, and don't install it on exit.
    pub fn skip_update(&self) {
        if let Some((r, _)) = self.update_ready.borrow_mut().take() {
            crate::update::skip(&r.version);
        }
        self.install_on_exit.set(false);
        self.update.set(UpdateUi::Hidden);
    }

    pub fn open_release_page(&self) {
        if let Some((r, _)) = self.update_ready.borrow().as_ref() {
            let _ = std::process::Command::new("explorer.exe").arg(&r.page).spawn();
        }
    }

    /// Everything that must happen before the window goes away.
    pub fn on_exit(&self) {
        self.commit_pending_delete();
        if self.install_on_exit.replace(false)
            && let Some((_, path)) = self.update_ready.borrow().as_ref()
        {
            let _ = crate::update::launch_installer(path, false);
        }
    }

    pub fn toast(&self, title: &str, body: &str) {
        self.toaster.show(Toast::new(title).body(body).duration(Duration::from_millis(2600)));
    }

    pub fn toast_error(&self, title: &str, body: &str) {
        self.toaster.show(
            Toast::new(title).body(body).kind(ToastKind::Error).duration(Duration::from_secs(5)),
        );
    }
}
