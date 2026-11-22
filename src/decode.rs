//! Turning a file into pixels, off the UI thread.
use crate::media::{Entry, MediaKind};
use std::{io::Cursor, path::Path, sync::Arc};
use zgx::image::ImageData;

/// The GPU's texture limit: bigger pictures are scaled down to fit it.
const MAX_SIDE: u32 = 16384;
/// SVGs rasterise with this many pixels on their long side.
const SVG_SIDE: f32 = 3072.;

pub struct Decoded {
    /// A still, or an animation's first frame carrying the animation.
    pub image: Arc<ImageData>,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
}

pub fn decode(entry: &Entry) -> Result<Decoded, String> {
    match entry.kind {
        MediaKind::Gif => decode_gif(&entry.path),
        MediaKind::Svg => decode_svg(&entry.path),
        MediaKind::Image => decode_raster(&entry.path),
        MediaKind::Video => Err("video".into()),
    }
}

fn still(width: u32, height: u32, rgba: Vec<u8>) -> Result<Decoded, String> {
    let image = ImageData::new(width, height, rgba).map_err(str::to_owned)?;
    Ok(Decoded { image: Arc::new(image), width, height, frames: 1 })
}

fn decode_raster(path: &Path) -> Result<Decoded, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match decode_with_image_crate(&bytes) {
        Ok(img) => {
            let img = fit_texture(img);
            let (w, h) = (img.width(), img.height());
            still(w, h, img.into_raw())
        }
        // AVIF, HEIC, JPEG XR and anything else Windows has a codec for.
        Err(first) => match crate::wic::decode_file(path) {
            Ok((w, h, rgba)) => {
                let img = image::RgbaImage::from_raw(w, h, rgba).ok_or("bad WIC buffer")?;
                let img = fit_texture(img);
                let (w, h) = (img.width(), img.height());
                still(w, h, img.into_raw())
            }
            Err(second) => Err(format!("{first}; {second}")),
        },
    }
}

fn decode_with_image_crate(bytes: &[u8]) -> Result<image::RgbaImage, String> {
    use image::{DynamicImage, ImageDecoder, ImageReader};
    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    Ok(img.into_rgba8())
}

fn fit_texture(img: image::RgbaImage) -> image::RgbaImage {
    let (w, h) = img.dimensions();
    if w <= MAX_SIDE && h <= MAX_SIDE {
        return img;
    }
    let s = MAX_SIDE as f32 / w.max(h) as f32;
    let (nw, nh) = (((w as f32 * s) as u32).max(1), ((h as f32 * s) as u32).max(1));
    image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Triangle)
}

fn decode_gif(path: &Path) -> Result<Decoded, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    match zgx_gpu::assets::decode_gif(&bytes) {
        Ok(animation) => {
            let frames = animation.len();
            let first = animation.frames()[0].image.clone();
            let (width, height) = (first.width(), first.height());
            let image = if frames > 1 { Arc::new(ImageData::animated(animation)) } else { first };
            Ok(Decoded { image, width, height, frames })
        }
        Err(_) => decode_raster(path),
    }
}

fn decode_svg(path: &Path) -> Result<Decoded, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let tree = usvg::Tree::from_data(&bytes, &usvg::Options::default()).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (sw, sh) = (size.width().max(1.), size.height().max(1.));
    let scale = SVG_SIDE / sw.max(sh);
    let (w, h) = (((sw * scale) as u32).max(1), ((sh * scale) as u32).max(1));
    let image = zgx_gpu::assets::decode_svg(&bytes, w, h).map_err(|e| format!("{e:?}"))?;
    Ok(Decoded { width: image.width(), height: image.height(), image, frames: 1 })
}

/// A copy of `image` turned a quarter `turns` times clockwise.
pub fn rotate(image: &ImageData, turns: i32) -> Option<Arc<ImageData>> {
    let buf = image::RgbaImage::from_raw(image.width(), image.height(), image.pixels().to_vec())?;
    let out = match turns.rem_euclid(4) {
        1 => image::imageops::rotate90(&buf),
        2 => image::imageops::rotate180(&buf),
        3 => image::imageops::rotate270(&buf),
        _ => buf,
    };
    let (w, h) = out.dimensions();
    ImageData::new(w, h, out.into_raw()).ok().map(Arc::new)
}
