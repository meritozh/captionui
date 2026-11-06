//! Time what the docs view pays per example: building the plot and running
//! its pipeline (`natural_height` prepares every mark, as `prepaint` does).
//!
//! `cargo run --release -p gpui-plot-demo --example bench` prints the slowest
//! examples and the total per page; `BENCH_FILTER=marks-dot` narrows it.

use std::time::Instant;

use gpui_plot_demo::examples;

fn main() {
    let filter = std::env::var("BENCH_FILTER").ok();
    let all = examples::examples();
    // Warm the dataset caches so loading a CSV once is not charged to the
    // first example that reads it.
    for example in &all {
        let _ = (example.build)();
    }
    let mut timings: Vec<(String, String, f64, f64)> = Vec::new();
    for example in &all {
        let id = example.id();
        if filter.as_ref().is_some_and(|f| !id.contains(f.as_str())) {
            continue;
        }
        let runs: usize = std::env::var("BENCH_RUNS").ok().and_then(|r| r.parse().ok()).unwrap_or(3);
        let start = Instant::now();
        for _ in 0..runs {
            let _ = (example.build)();
        }
        let build = start.elapsed().as_secs_f64() * 1e3 / runs as f64;
        let plot = (example.build)();
        let start = Instant::now();
        for _ in 0..runs {
            let _ = plot.natural_height(640.);
        }
        let pipeline = start.elapsed().as_secs_f64() * 1e3 / runs as f64;
        timings.push((id.to_string(), format!("{}/{}", example.section, example.page), build, pipeline));
    }
    timings.sort_by(|a, b| (b.2 + b.3).total_cmp(&(a.2 + a.3)));
    println!("slowest examples (ms: build, pipeline):");
    for (id, _, build, pipeline) in timings.iter().take(20) {
        println!("  {id:32} {build:8.2} {pipeline:8.2}");
    }
    let mut pages: Vec<(String, f64, usize)> = Vec::new();
    for (_, page, build, pipeline) in &timings {
        match pages.iter_mut().find(|(p, _, _)| p == page) {
            Some(entry) => {
                entry.1 += build + pipeline;
                entry.2 += 1;
            }
            None => pages.push((page.clone(), build + pipeline, 1)),
        }
    }
    pages.sort_by(|a, b| b.1.total_cmp(&a.1));
    println!("slowest pages (ms per full re-render of every plot, examples):");
    for (page, total, n) in pages.iter().take(12) {
        println!("  {page:32} {total:8.1} {n:4}");
    }
    let total: f64 = timings.iter().map(|t| t.2 + t.3).sum();
    println!("all {} examples: {total:.1} ms", timings.len());
}
