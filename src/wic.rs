//! Windows Imaging Component: decodes whatever codecs Windows has
//! (AVIF and HEIC through the Store extensions, JPEG XR, odd TIFFs).
use std::path::Path;
use windows::{
    Win32::{
        Foundation::GENERIC_READ,
        Graphics::Imaging::*,
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_MULTITHREADED, CoCreateInstance,
            CoInitializeEx,
        },
    },
    core::HSTRING,
};

/// Join this thread to a single-threaded apartment (for shell thumbnail
/// providers, which expect one). Call before any other COM use on it.
pub fn com_init_sta() {
    thread_local!(static INIT: () = {
        // SAFETY: first COM call on this thread.
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    });
    INIT.with(|_| {});
}

/// Join this thread to the multithreaded apartment once.
pub fn com_init() {
    thread_local!(static INIT: () = {
        // SAFETY: called once per thread; S_FALSE/RPC_E_CHANGED_MODE are fine.
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    });
    INIT.with(|_| {});
}

pub fn factory() -> windows::core::Result<IWICImagingFactory> {
    com_init();
    // SAFETY: plain COM activation.
    unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER) }
}

/// Straight-alpha RGBA8 pixels of the file's first frame.
pub fn decode_file(path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let run = || -> windows::core::Result<(u32, u32, Vec<u8>)> {
        let f = factory()?;
        // SAFETY: COM calls on valid interfaces; the buffer is sized w*h*4.
        unsafe {
            let decoder = f.CreateDecoderFromFilename(
                &HSTRING::from(path.as_os_str()),
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let converter = f.CreateFormatConverter()?;
            converter.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.,
                WICBitmapPaletteTypeCustom,
            )?;
            let (mut w, mut h) = (0, 0);
            converter.GetSize(&mut w, &mut h)?;
            let mut buf = vec![0u8; w as usize * h as usize * 4];
            converter.CopyPixels(std::ptr::null(), w * 4, &mut buf)?;
            Ok((w, h, buf))
        }
    };
    run().map_err(|e| format!("Windows has no decoder for this file ({})", e.message()))
}
