//! Score each snapshot against Plot's reference render: both are reduced to
//! a coarse grayscale "ink" grid at a shared aspect, and the score is the
//! fraction of cells whose ink agrees. Writes a ranked report.
//!
//! `REFERENCE_DIR=/tmp/plot-scripts/ref cargo run -p gpui-plot-demo --example compare`

use std::path::PathBuf;

const GRID: u32 = 48;

fn ink(path: &std::path::Path, trim: bool) -> Option<Vec<f32>> {
    let image = image::open(path).ok()?.to_luma8();
    let (w, h) = image.dimensions();
    // Trim the white border so both renders frame the chart alike.
    let mut bounds = (w, h, 0, 0);
    for (x, y, p) in image.enumerate_pixels() {
        if p.0[0] < 235 {
            bounds = (
                bounds.0.min(x),
                bounds.1.min(y),
                bounds.2.max(x),
                bounds.3.max(y),
            );
        }
    }
    if !trim || bounds.2 <= bounds.0 || bounds.3 <= bounds.1 {
        bounds = (0, 0, w - 1, h - 1);
    }
    let (x0, y0, x1, y1) = bounds;
    let crop = image::imageops::crop_imm(&image, x0, y0, x1 - x0 + 1, y1 - y0 + 1).to_image();
    let small = image::imageops::resize(&crop, GRID, GRID, image::imageops::FilterType::Triangle);
    Some(small.pixels().map(|p| 1. - p.0[0] as f32 / 255.).collect())
}

fn main() {
    let reference = PathBuf::from(std::env::var("REFERENCE_DIR").expect("REFERENCE_DIR"));
    let ours_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("snapshots");
    let mut scores = Vec::new();
    for entry in std::fs::read_dir(&ours_dir).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let (Some(a), Some(b)) = (ink(&entry.path(), true), ink(&reference.join(&name), true))
        else {
            continue;
        };
        // Correlation of the two ink grids.
        let n = a.len() as f32;
        let (ma, mb) = (a.iter().sum::<f32>() / n, b.iter().sum::<f32>() / n);
        let (mut num, mut da, mut db) = (0., 0., 0.);
        for (x, y) in a.iter().zip(&b) {
            num += (x - ma) * (y - mb);
            da += (x - ma).powi(2);
            db += (y - mb).powi(2);
        }
        let r = if da > 0. && db > 0. {
            num / (da * db).sqrt()
        } else {
            0.
        };
        scores.push((r, name.trim_end_matches(".png").to_string()));
    }
    scores.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (r, name) in &scores {
        println!("{r:.3} {name}");
    }
    let mean = scores.iter().map(|s| s.0).sum::<f32>() / scores.len().max(1) as f32;
    eprintln!("{} compared, mean {mean:.3}", scores.len());
}
