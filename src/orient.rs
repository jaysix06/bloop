//! Saving a rotation to disk: lossless for JPEGs with an EXIF orientation
//! tag (only two bytes change), re-encoded for everything else.
use std::path::Path;

/// EXIF orientation for 0, 90, 180, 270 degrees clockwise.
const ORIENT: [u16; 4] = [1, 6, 3, 8];

pub enum Saved {
    Lossless,
    Reencoded,
}

pub fn save_rotation(path: &Path, turns: i32) -> Result<Saved, String> {
    let turns = turns.rem_euclid(4);
    if turns == 0 {
        return Ok(Saved::Lossless);
    }
    let mut bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    if let Some(at) = find_orientation(&bytes) {
        let (pos, big_endian) = at;
        let read = if big_endian {
            u16::from_be_bytes([bytes[pos], bytes[pos + 1]])
        } else {
            u16::from_le_bytes([bytes[pos], bytes[pos + 1]])
        };
        if let Some(current) = ORIENT.iter().position(|o| *o == read) {
            let next = ORIENT[(current + turns as usize) % 4];
            let b = if big_endian { next.to_be_bytes() } else { next.to_le_bytes() };
            bytes[pos..pos + 2].copy_from_slice(&b);
            write_atomic(path, &bytes)?;
            return Ok(Saved::Lossless);
        }
    }
    reencode(path, &bytes, turns)
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("bloop-tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

fn reencode(path: &Path, bytes: &[u8], turns: i32) -> Result<Saved, String> {
    use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader};
    let reader = ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let format = reader.format().ok_or("unknown format")?;
    let mut decoder = reader.into_decoder().map_err(|e| e.to_string())?;
    let orientation = decoder.orientation().ok();
    let mut img = DynamicImage::from_decoder(decoder).map_err(|e| e.to_string())?;
    if let Some(o) = orientation {
        img.apply_orientation(o);
    }
    let img = match turns {
        1 => img.rotate90(),
        2 => img.rotate180(),
        3 => img.rotate270(),
        _ => img,
    };
    let mut out = std::io::Cursor::new(Vec::new());
    match format {
        ImageFormat::Jpeg => {
            let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 95);
            img.to_rgb8().write_with_encoder(enc).map_err(|e| e.to_string())?;
        }
        ImageFormat::Png | ImageFormat::Bmp | ImageFormat::Tiff | ImageFormat::Tga
        | ImageFormat::WebP | ImageFormat::Ico => {
            img.write_to(&mut out, format).map_err(|e| e.to_string())?;
        }
        _ => return Err("Bloop can't save this format".into()),
    }
    write_atomic(path, out.get_ref())?;
    Ok(Saved::Reencoded)
}

/// Byte offset of IFD0's orientation value in a JPEG, and its endianness.
fn find_orientation(b: &[u8]) -> Option<(usize, bool)> {
    if b.get(0..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut i = 2;
    while i + 4 <= b.len() {
        if b[i] != 0xFF {
            return None;
        }
        let marker = b[i + 1];
        if marker == 0xDA || marker == 0xD9 {
            return None;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        let seg = i + 4;
        if marker == 0xE1 && b.get(seg..seg + 6)? == b"Exif\0\0" {
            let tiff = seg + 6;
            let be = match b.get(tiff..tiff + 2)? {
                b"MM" => true,
                b"II" => false,
                _ => return None,
            };
            let u16_at = |p: usize| -> Option<u16> {
                let s = [*b.get(p)?, *b.get(p + 1)?];
                Some(if be { u16::from_be_bytes(s) } else { u16::from_le_bytes(s) })
            };
            let u32_at = |p: usize| -> Option<u32> {
                let s = [*b.get(p)?, *b.get(p + 1)?, *b.get(p + 2)?, *b.get(p + 3)?];
                Some(if be { u32::from_be_bytes(s) } else { u32::from_le_bytes(s) })
            };
            let ifd = tiff + u32_at(tiff + 4)? as usize;
            let count = u16_at(ifd)? as usize;
            for n in 0..count {
                let e = ifd + 2 + n * 12;
                if u16_at(e)? == 0x0112 && u16_at(e + 2)? == 3 {
                    return Some((e + 8, be));
                }
            }
            return None;
        }
        i = seg + len - 2;
    }
    None
}
