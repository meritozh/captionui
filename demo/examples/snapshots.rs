//! Renders every ported documentation example offscreen and writes it to
//! `demo/snapshots/<id>.png`.
//!
//! This is the check that the charts actually paint: the unit tests cover the
//! model (scales, ticks, transforms, resolution), and this covers the pixels.
//!
//! ```sh
//! cargo run -p gpui-plot-demo --example snapshots
//! SNAPSHOT_FILTER=marks-dot cargo run -p gpui-plot-demo --example snapshots
//! ```
//!
//! With `REFERENCE_DIR` set to a folder of Plot's own renders (`<id>.png`),
//! it also writes `<COMPARE_DIR>/<id>.png`: this port on the left, Plot on the
//! right, for review.

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AppContext, Context, HeadlessAppContext, IntoElement, ParentElement, Render, Styled, Window,
    div, px, size,
};
use gpui_plot::Plot;
use gpui_plot_demo::examples;

/// Plot's default width, which the docs' examples are drawn at.
const WIDTH: f32 = 640.;
/// The padding around the chart in the captured image.
const PAD: f32 = 16.;

struct Snapshot {
    build: fn() -> Plot,
    height: f32,
}

impl Render for Snapshot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().colors.background)
            .p(px(PAD))
            .child(div().w(px(WIDTH)).h(px(self.height)).child((self.build)()))
    }
}

/// A headless context with GPUI's real offscreen renderer (Metal on macOS,
/// WGPU on Linux), so what it captures is what the window would show.
fn context() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        std::sync::Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    cx
}

/// The height an example draws at: its plot's natural height at Plot's
/// default width, as Plot's `autoHeight` lays it out.
fn height_of(example: &examples::Example) -> f32 {
    (example.build)().natural_height(WIDTH)
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("snapshots");
    std::fs::create_dir_all(&out_dir).expect("failed to create the snapshots directory");
    let reference = std::env::var("REFERENCE_DIR").ok().map(PathBuf::from);
    let compare = std::env::var("COMPARE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir().join("plot-compare"));
    if reference.is_some() {
        std::fs::create_dir_all(&compare).expect("failed to create the compare directory");
    }

    let filter = std::env::var("SNAPSHOT_FILTER").ok();
    let all = examples::examples();
    let selected: Vec<_> = all
        .iter()
        .filter(|example| {
            filter
                .as_ref()
                .is_none_or(|filter| example.id().contains(filter.as_str()))
        })
        .collect();
    if selected.is_empty() {
        println!("no examples match");
        return;
    }

    let mut cx = context();
    // Tall enough for the tallest example; each capture is trimmed to its own.
    let window_height = 1400.;
    let (handle, entity) = cx
        .update(|cx| {
            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds {
                        origin: Default::default(),
                        size: size(px(WIDTH + PAD * 2.), px(window_height)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| Snapshot {
                        build: selected[0].build,
                        height: 400.,
                    })
                },
            )
        })
        .expect("failed to open the snapshot window");

    let mut failures = Vec::new();
    for example in &selected {
        let id = example.id();
        let height = height_of(example).min(window_height - PAD * 2.);
        let built = std::panic::catch_unwind(|| (example.build)());
        if built.is_err() {
            failures.push(format!("{id}: panicked while building"));
            continue;
        }
        let rendered = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cx.update_window(handle, |_, window, cx| {
                entity.update(cx, |snapshot, _| {
                    snapshot.build = example.build;
                    snapshot.height = height;
                });
                // The first frames start the appear animation; advancing the
                // clock past it captures the settled chart.
                for _ in 0..2 {
                    window.draw(cx).clear(cx);
                }
                cx.background_executor()
                    .advance_clock(Duration::from_secs(2));
                window.draw(cx).clear(cx);
            })
            .expect("failed to render the snapshot frame");
            cx.capture_screenshot(handle)
                .expect("offscreen rendering must be available")
        }));
        let Ok(captured) = rendered else {
            failures.push(format!("{id}: panicked while painting"));
            continue;
        };
        let scale = captured.width() as f32 / (WIDTH + PAD * 2.);
        let crop = ((height + PAD * 2.) * scale).round() as u32;
        let image = image::imageops::crop_imm(
            &captured,
            0,
            0,
            captured.width(),
            crop.min(captured.height()),
        )
        .to_image();
        let path = out_dir.join(format!("{id}.png"));
        image
            .save(&path)
            .unwrap_or_else(|error| panic!("failed to save {}: {error}", path.display()));

        let colors = distinct_colors(&image);
        println!("{id:<36} {colors:>5} colors");
        // A monochrome chart has few colors but still paints: only an image
        // of the background alone fails.
        if colors < 2 {
            failures.push(format!("{id}: painted almost nothing"));
        }
        if let Some(reference) = &reference {
            side_by_side(
                &image,
                &reference.join(format!("{id}.png")),
                &compare.join(format!("{id}.png")),
            );
        }
    }

    let missing = examples::missing();
    println!(
        "\n{} rendered, {} failed, {} of {} docs examples not ported yet",
        selected.len() - failures.len(),
        failures.len(),
        missing.len(),
        examples::docs().len()
    );
    if !failures.is_empty() {
        panic!("snapshots failed:\n{}", failures.join("\n"));
    }
}

/// Write this port's render and Plot's side by side, scaled to one width.
fn side_by_side(ours: &image::RgbaImage, reference: &Path, out: &Path) {
    let Ok(theirs) = image::open(reference) else {
        return;
    };
    let theirs = theirs.to_rgba8();
    let width = ours.width();
    let scaled = |image: &image::RgbaImage| {
        let height = (width as f32 * image.height() as f32 / image.width() as f32).round() as u32;
        image::imageops::resize(
            image,
            width,
            height.max(1),
            image::imageops::FilterType::Triangle,
        )
    };
    let theirs = scaled(&theirs);
    let height = ours.height().max(theirs.height());
    let mut sheet =
        image::RgbaImage::from_pixel(width * 2 + 8, height, image::Rgba([200, 200, 200, 255]));
    image::imageops::overlay(&mut sheet, ours, 0, 0);
    image::imageops::overlay(&mut sheet, &theirs, (width + 8) as i64, 0);
    let _ = sheet.save(out);
}

/// The number of distinct colors in an image.
fn distinct_colors(image: &image::RgbaImage) -> usize {
    use std::collections::HashSet;
    image
        .pixels()
        .map(|pixel| pixel.0)
        .collect::<HashSet<_>>()
        .len()
}
