//! Regenerates every icon raster derived from the project logo:
//! - pc/assets/tray_icon.png            (256px, build.rs bakes 32/64 tray pixmaps)
//! - android res/mipmap-*/              (legacy launcher + adaptive foreground)
//! - android res/drawable-*/ic_stat_airpipe.png (notification silhouette)
//!
//! Run from anywhere: `cargo run --release --manifest-path tools/icongen/Cargo.toml`
//! Paths are anchored to the main repo root (parent of tools/).

use image::imageops::{overlay, resize, FilterType};
use image::{GenericImageView, Rgba, RgbaImage};
use std::path::PathBuf;

/// Base dp -> px multipliers for the five Android densities.
const DENSITIES: [(&str, u32); 5] = [("mdpi", 1), ("hdpi", 2), ("xhdpi", 3), ("xxhdpi", 4), ("xxxhdpi", 6)];

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let logo_path = root.join("assets/airpipe_logo.png");
    let img = image::open(&logo_path)
        .unwrap_or_else(|e| panic!("open {}: {}", logo_path.display(), e))
        .to_rgba8();

    let mark = opaque_bbox_crop(&img);
    let pc_assets = root.join("pc/assets");
    let res = root.join("android/app/src/main/res");

    // PC tray icon source (256px, full original canvas)
    let tray = resize(&img, 256, 256, FilterType::Lanczos3);
    save(&tray, &pc_assets.join("tray_icon.png"));

    for &(d, mult) in &DENSITIES {
        // Legacy launcher icon: 48dp, transparent, as-is
        let px = 48 * mult;
        save(&resize(&img, px, px, FilterType::Lanczos3), &res.join(format!("mipmap-{d}/ic_launcher.png")));

        // Adaptive foreground: mark centered at 60% of the 108dp canvas (safe zone = 66/108)
        let px = 108 * mult;
        save(&centered(&mark, px, 0.60), &res.join(format!("mipmap-{d}/ic_launcher_foreground.png")));

        // Notification silhouette: white, alpha from source, mark at 85% of 24dp
        let px = 24 * mult;
        let base = centered(&mark, px, 0.85);
        let sil = RgbaImage::from_fn(px, px, |x, y| Rgba([255, 255, 255, base.get_pixel(x, y)[3]]));
        save(&sil, &res.join(format!("drawable-{d}/ic_stat_airpipe.png")));
    }

    println!("icon rasters regenerated from {}", logo_path.display());
}

/// Crop to the bounding box of pixels with alpha >= 16 (logo is a transparent mark).
fn opaque_bbox_crop(img: &RgbaImage) -> RgbaImage {
    let (w, h) = img.dimensions();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0u32, 0u32);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] >= 16 {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
    }
    assert!(x1 >= x0 && y1 >= y0, "logo has no visible pixels");
    img.view(x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image()
}

/// Scale `src` so its longest side is `fill` of `canvas_px`, centered on a transparent square.
fn centered(src: &RgbaImage, canvas_px: u32, fill: f64) -> RgbaImage {
    let target = ((canvas_px as f64) * fill).round() as u32;
    let (sw, sh) = src.dimensions();
    let scale = target as f64 / sw.max(sh) as f64;
    let (tw, th) = ((sw as f64 * scale).round() as u32, (sh as f64 * scale).round() as u32);
    let scaled = resize(src, tw.max(1), th.max(1), FilterType::Lanczos3);
    let mut canvas = RgbaImage::from_pixel(canvas_px, canvas_px, Rgba([0, 0, 0, 0]));
    overlay(&mut canvas, &scaled, ((canvas_px - tw) / 2) as i64, ((canvas_px - th) / 2) as i64);
    canvas
}

fn save(img: &RgbaImage, path: &PathBuf) {
    std::fs::create_dir_all(path.parent().unwrap()).expect("create dir");
    img.save(path).unwrap_or_else(|e| panic!("save {}: {}", path.display(), e));
    println!("  {}", path.display());
}
