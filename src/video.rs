//! Video through Windows Media Foundation's Media Engine in frame-server
//! mode: MF decodes, keeps audio in sync and plays it; each new frame is
//! copied out through a WIC bitmap and handed to the UI as an `ImageData`.
use std::{
    path::PathBuf,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::RECT,
        Graphics::{Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM, Imaging::*},
        Media::MediaFoundation::*,
        System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
    },
    core::BSTR,
};
use zgx::image::ImageData;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VideoStatus {
    pub duration: f64,
    pub position: f64,
    pub playing: bool,
    pub ended: bool,
    pub width: u32,
    pub height: u32,
}

pub enum VideoMsg {
    Frame(u64, Arc<ImageData>),
    Status(u64, VideoStatus),
    Error(u64, String),
}

pub enum VideoCmd {
    Toggle,
    Seek(f64),
    Volume(f64),
    Loop(bool),
}

pub struct VideoPlayer {
    tx: mpsc::Sender<Option<VideoCmd>>,
}

impl VideoPlayer {
    pub fn open(
        path: PathBuf,
        generation: u64,
        out: async_channel::Sender<VideoMsg>,
        volume: f64,
        looping: bool,
    ) -> Self {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("bloop-video".into())
            .spawn(move || {
                if let Err(e) = run(&path, generation, &out, rx, volume, looping) {
                    let _ = out.try_send(VideoMsg::Error(generation, e.message().to_string()));
                }
            })
            .expect("spawn video thread");
        Self { tx }
    }
    pub fn send(&self, cmd: VideoCmd) {
        let _ = self.tx.send(Some(cmd));
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        let _ = self.tx.send(None);
    }
}

#[windows_core::implement(IMFMediaEngineNotify)]
struct Notify {
    out: async_channel::Sender<VideoMsg>,
    generation: u64,
}

impl IMFMediaEngineNotify_Impl for Notify_Impl {
    fn EventNotify(&self, event: u32, param1: usize, param2: u32) -> windows_core::Result<()> {
        if event == MF_MEDIA_ENGINE_EVENT_ERROR.0 as u32 {
            // 0xC00D36xx: Media Foundation has no source or decoder for it.
            // param1 is MF_MEDIA_ENGINE_ERR: 3 = decode, 4 = source not supported.
            let msg = if matches!(param1, 3 | 4) || param2 & 0xFFFF_FF00 == 0xC00D_3600 {
                "codec".to_string()
            } else {
                format!("Playback failed (0x{param2:08X})")
            };
            let _ = self.out.try_send(VideoMsg::Error(self.generation, msg));
        }
        Ok(())
    }
}

fn run(
    path: &std::path::Path,
    generation: u64,
    out: &async_channel::Sender<VideoMsg>,
    rx: mpsc::Receiver<Option<VideoCmd>>,
    volume: f64,
    looping: bool,
) -> windows_core::Result<()> {
    crate::wic::com_init();
    // SAFETY: Media Foundation and COM calls on this thread's own objects;
    // the engine is shut down before they drop.
    unsafe {
        MFStartup(MF_VERSION, MFSTARTUP_FULL)?;
        let result = (|| -> windows_core::Result<()> {
            let factory: IMFMediaEngineClassFactory =
                CoCreateInstance(&CLSID_MFMediaEngineClassFactory, None, CLSCTX_INPROC_SERVER)?;
            let mut attrs = None;
            MFCreateAttributes(&mut attrs, 2)?;
            let attrs = attrs.expect("attributes");
            let notify: IMFMediaEngineNotify = Notify { out: out.clone(), generation }.into();
            attrs.SetUnknown(&MF_MEDIA_ENGINE_CALLBACK, &notify)?;
            attrs.SetUINT32(&MF_MEDIA_ENGINE_VIDEO_OUTPUT_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM.0 as u32)?;
            let engine = factory.CreateInstance(0, &attrs)?;
            let wic = crate::wic::factory()?;
            engine.SetVolume(volume)?;
            engine.SetLoop(looping)?;
            engine.SetAutoPlay(true)?;
            engine.SetSource(&BSTR::from(path.as_os_str().to_string_lossy().as_ref()))?;

            let mut bitmap: Option<(u32, u32, IWICBitmap)> = None;
            let mut last_pts = i64::MIN;
            let mut last_status = Instant::now() - Duration::from_secs(1);
            let mut status = VideoStatus::default();
            'run: loop {
                loop {
                    match rx.try_recv() {
                        Ok(Some(cmd)) => match cmd {
                            VideoCmd::Toggle => {
                                if engine.IsPaused().as_bool() || engine.IsEnded().as_bool() {
                                    engine.Play()?
                                } else {
                                    engine.Pause()?
                                }
                            }
                            VideoCmd::Seek(t) => {
                                let _ = engine.SetCurrentTime(t.max(0.));
                            }
                            VideoCmd::Volume(v) => engine.SetVolume(v.clamp(0., 1.))?,
                            VideoCmd::Loop(l) => engine.SetLoop(l)?,
                        },
                        Ok(None) | Err(mpsc::TryRecvError::Disconnected) => break 'run,
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if engine.HasVideo().as_bool() {
                    if let Ok(pts) = engine.OnVideoStreamTick() {
                        if pts != last_pts && pts >= 0 {
                            last_pts = pts;
                            let (mut w, mut h) = (0u32, 0u32);
                            engine.GetNativeVideoSize(Some(&mut w), Some(&mut h))?;
                            if w > 0 && h > 0 {
                                if bitmap.as_ref().is_none_or(|(bw, bh, _)| (*bw, *bh) != (w, h)) {
                                    let b = wic.CreateBitmap(
                                        w,
                                        h,
                                        &GUID_WICPixelFormat32bppPBGRA,
                                        WICBitmapCacheOnLoad,
                                    )?;
                                    bitmap = Some((w, h, b));
                                }
                                let (_, _, b) = bitmap.as_ref().unwrap();
                                let dst = RECT { left: 0, top: 0, right: w as i32, bottom: h as i32 };
                                if engine.TransferVideoFrame(b, None, &dst, None).is_ok() {
                                    if let Some(frame) = copy_out(b, w, h) {
                                        let _ = out.try_send(VideoMsg::Frame(generation, frame));
                                    }
                                }
                                status.width = w;
                                status.height = h;
                            }
                        }
                    }
                }
                if last_status.elapsed() >= Duration::from_millis(100) {
                    last_status = Instant::now();
                    let d = engine.GetDuration();
                    status.duration = if d.is_finite() { d } else { 0. };
                    status.position = engine.GetCurrentTime();
                    status.playing = !engine.IsPaused().as_bool() && !engine.IsEnded().as_bool();
                    status.ended = engine.IsEnded().as_bool();
                    let _ = out.try_send(VideoMsg::Status(generation, status));
                }
                thread::sleep(Duration::from_millis(4));
            }
            let _ = engine.Shutdown();
            Ok(())
        })();
        let _ = MFShutdown();
        result
    }
}

fn copy_out(bitmap: &IWICBitmap, w: u32, h: u32) -> Option<Arc<ImageData>> {
    // SAFETY: the lock's pointer is valid for `size` bytes until it drops.
    unsafe {
        let rect = WICRect { X: 0, Y: 0, Width: w as i32, Height: h as i32 };
        let lock = bitmap.Lock(&rect, WICBitmapLockRead.0 as u32).ok()?;
        let stride = lock.GetStride().ok()? as usize;
        let mut size = 0u32;
        let mut ptr = std::ptr::null_mut();
        lock.GetDataPointer(&mut size, &mut ptr).ok()?;
        let src = std::slice::from_raw_parts(ptr, size as usize);
        let row = w as usize * 4;
        let mut rgba = vec![0u8; row * h as usize];
        for y in 0..h as usize {
            let s = &src[y * stride..y * stride + row];
            let d = &mut rgba[y * row..(y + 1) * row];
            for (dp, sp) in d.chunks_exact_mut(4).zip(s.chunks_exact(4)) {
                dp[0] = sp[2];
                dp[1] = sp[1];
                dp[2] = sp[0];
                dp[3] = 255;
            }
        }
        drop(lock);
        ImageData::new(w, h, rgba).ok().map(Arc::new)
    }
}
