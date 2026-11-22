// Bloop's icon art, shared by the app's and the installer's build scripts
// (`include!`d, so it needs the `image` build-dependency in scope).

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn lerp(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Signed distance to a rounded square ("squircle-ish") of half size `h`, radius `r`.
fn rounded_box(x: f32, y: f32, h: f32, r: f32) -> f32 {
    let qx = x.abs() - h + r;
    let qy = y.abs() - h + r;
    let outside = (qx.max(0.).powi(2) + qy.max(0.).powi(2)).sqrt();
    outside + qx.max(qy).min(0.) - r
}

fn render(size: u32) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let s = size as f32;
    ImageBuffer::from_fn(size, size, |px, py| {
        // Normalised coordinates in [-1, 1].
        let x = (px as f32 + 0.5) / s * 2. - 1.;
        let y = (py as f32 + 0.5) / s * 2. - 1.;
        let aa = 2.5 / s;
        // Tile: deep ink rounded square.
        let d_tile = rounded_box(x, y, 0.94, 0.42);
        let tile_a = 1. - smoothstep(-aa, aa, d_tile);
        // Background gradient of the tile.
        let tile = lerp([18., 16., 28.], [30., 24., 48.], (y + 1.) / 2.);
        // The bloop: a blob made of two merged circles.
        let c1 = ((x + 0.08).powi(2) + (y - 0.10).powi(2)).sqrt() - 0.50;
        let c2 = ((x - 0.26).powi(2) + (y + 0.30).powi(2)).sqrt() - 0.24;
        let k = 0.22;
        let hmix = (0.5 + 0.5 * (c2 - c1) / k).clamp(0., 1.);
        let d_blob = c2 + (c1 - c2) * hmix - k * hmix * (1. - hmix);
        let blob_a = 1. - smoothstep(-aa, aa, d_blob);
        let t = ((x - y) * 0.5 + 0.5).clamp(0., 1.);
        let blob = lerp([255., 122., 89.], [139., 92., 246.], t);
        // Highlight.
        let hl = ((x + 0.26).powi(2) + (y - 0.30).powi(2) * 1.4).sqrt() - 0.12;
        let hl_a = (1. - smoothstep(-aa * 2., aa * 6., hl)) * 0.85;
        let mut rgb = lerp(tile, blob, blob_a);
        rgb = lerp(rgb, [255., 255., 255.], hl_a * blob_a);
        Rgba([rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, (tile_a * 255.) as u8])
    })
}

/// Write the multi-size .ico (and a 128 px PNG) into `out`; returns the .ico path.
fn write_icon(out: &std::path::Path) -> std::path::PathBuf {
    use image::imageops::FilterType;
    let master = render(1024);
    let frames: Vec<_> = [16u32, 24, 32, 48, 64, 128, 256]
        .iter()
        .map(|&n| {
            let img = image::imageops::resize(&master, n, n, FilterType::Lanczos3);
            image::codecs::ico::IcoFrame::as_png(img.as_raw(), n, n, image::ExtendedColorType::Rgba8)
                .unwrap()
        })
        .collect();
    let ico = out.join("bloop.ico");
    let file = std::fs::File::create(&ico).unwrap();
    image::codecs::ico::IcoEncoder::new(file).encode_images(&frames).unwrap();
    image::imageops::resize(&master, 128, 128, FilterType::Lanczos3)
        .save(out.join("bloop-128.png"))
        .unwrap();
    ico
}
