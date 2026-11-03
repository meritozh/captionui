//! Renders every gallery example offscreen and writes it to `snapshots/`.
//!
//! This is the check that the charts actually paint: the unit tests cover the
//! model (scales, ticks, transforms, resolution), and this covers the pixels.
//! Run it with:
//!
//! ```sh
//! cargo run -p gpui-plot-demo --example snapshots
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{
    AppContext, Context, HeadlessAppContext, IntoElement, ParentElement, Render, Styled, Window,
    div, px, size,
};
use gpui_plot::Plot;
use gpui_plot_demo::gallery;

/// The window the snapshot renderer draws into: one chart on a plain
/// background, rebuilt every frame from its example's builder.
struct Snapshot {
    build: fn() -> Plot,
}

impl Render for Snapshot {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(cx.theme().colors.background)
            .p_4()
            .child((self.build)().height(440.))
    }
}

/// Build a headless context with GPUI's real offscreen renderer (Metal on
/// macOS, WGPU on Linux), so what it captures is what the window would show.
fn context() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        std::sync::Arc::new(gpui_kit::assets::Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    cx
}

fn main() {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("snapshots");
    std::fs::create_dir_all(&out_dir).expect("failed to create the snapshots directory");

    let mut cx = context();
    let examples = gallery::examples();

    let (handle, entity) = cx
        .update(|cx| {
            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(gpui_kit::Bounds {
                        origin: Default::default(),
                        size: size(px(760.), px(520.)),
                    })),
                    focus: false,
                    show: false,
                    ..Default::default()
                },
                cx,
                |_, cx| {
                    cx.new(|_| Snapshot {
                        build: examples[0].build,
                    })
                },
            )
        })
        .expect("failed to open the snapshot window");

    let mut failures = Vec::new();

    for example in &examples {
        cx.update_window(handle, |_, window, cx| {
            entity.update(cx, |snapshot, _| snapshot.build = example.build);
            // The first frame starts the appear animation; advancing the test
            // clock past it and drawing again captures the settled chart
            // rather than the first frame of the draw-in.
            for _ in 0..3 {
                window.draw(cx).clear(cx);
            }
            cx.background_executor()
                .advance_clock(Duration::from_secs(2));
            window.draw(cx).clear(cx);
            cx.background_executor()
                .advance_clock(Duration::from_secs(2));
            window.draw(cx).clear(cx);
        })
        .expect("failed to render the snapshot frame");

        let image = cx
            .capture_screenshot(handle)
            .expect("offscreen rendering must be available");

        let colors = distinct_colors(&image);
        let path = out_dir.join(format!("{}.png", slug(example.title)));
        image
            .save(&path)
            .unwrap_or_else(|error| panic!("failed to save {}: {error}", path.display()));

        println!(
            "{:<22} {:>4} colors  {}",
            example.title,
            colors,
            path.display()
        );
        if colors < 12 {
            failures.push(example.title);
        }
    }

    if failures.is_empty() {
        println!("\nall {} examples painted", examples.len());
        let sheet = out_dir.join("contact-sheet.png");
        contact_sheet(&out_dir, &examples, &sheet);
        println!("contact sheet: {}", sheet.display());
    } else {
        panic!("examples painted almost nothing: {failures:?}");
    }
}

/// Tile every snapshot into one image, so the whole gallery is reviewable at
/// a glance.
fn contact_sheet(dir: &Path, examples: &[gallery::Example], out: &Path) {
    const COLUMNS: u32 = 3;
    const TILE: (u32, u32) = (620, 424);

    let rows = (examples.len() as u32).div_ceil(COLUMNS);
    let mut sheet = image::RgbaImage::new(TILE.0 * COLUMNS, TILE.1 * rows);
    for (index, example) in examples.iter().enumerate() {
        let path = dir.join(format!("{}.png", slug(example.title)));
        let Ok(image) = image::open(&path) else {
            continue;
        };
        let tile = image::imageops::resize(
            &image.to_rgba8(),
            TILE.0,
            TILE.1,
            image::imageops::FilterType::Lanczos3,
        );
        let x = (index as u32 % COLUMNS) * TILE.0;
        let y = (index as u32 / COLUMNS) * TILE.1;
        image::imageops::overlay(&mut sheet, &tile, x as i64, y as i64);
    }
    sheet
        .save(out)
        .unwrap_or_else(|error| panic!("failed to save {}: {error}", out.display()));
}

/// The number of distinct colors in an image.
fn distinct_colors(image: &image::RgbaImage) -> usize {
    use std::collections::HashSet;
    let mut colors = HashSet::new();
    for pixel in image.pixels() {
        colors.insert([pixel[0], pixel[1], pixel[2], pixel[3]]);
    }
    colors.len()
}

fn slug(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}
