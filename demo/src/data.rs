//! The demo's sample datasets: the shapes Plot's own examples use.

use chrono::{Duration, NaiveDate, TimeZone, Utc};
use gpui_plot::{Table, Value};

/// A deterministic pseudo-random generator, so every run shows the same data.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_f64(&mut self) -> f64 {
        // xorshift64*, which is small, fast and good enough for a demo.
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        ((x.wrapping_mul(0x2545F4914F6CDD1D)) >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn next_in(&mut self, min: f64, max: f64) -> f64 {
        min + self.next_f64() * (max - min)
    }

    /// A value from the standard normal distribution, by Box–Muller.
    pub fn gaussian(&mut self) -> f64 {
        let u1 = self.next_f64().max(f64::MIN_POSITIVE);
        let u2 = self.next_f64();
        (-2. * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// Monthly observations of a few economic series, the shape of Plot's
/// "Monthly economic indicators" example.
pub fn economics() -> Vec<Table> {
    let start = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
    let mut rng = Rng::new(7);
    let mut unemployment = 3.6;
    let mut inflation = 2.0;

    (0..60)
        .map(|month| {
            let date = start + Duration::days(30 * month);
            unemployment = (unemployment + rng.next_in(-0.25, 0.3)).clamp(2.5, 14.);
            inflation = (inflation + rng.next_in(-0.2, 0.22)).clamp(-1., 9.);
            Table::empty()
                .with("date", Value::date(date))
                .with("unemployment", Value::Number(round1(unemployment)))
                .with("inflation", Value::Number(round1(inflation)))
                .with(
                    "gdp",
                    Value::Number(round1(1800. + month as f64 * 8. + rng.next_in(-40., 40.))),
                )
        })
        .collect()
}

/// Daily temperature readings with a seasonal cycle, the shape of Plot's
/// "Line chart" example.
pub fn temperature() -> Vec<Table> {
    let start = NaiveDate::from_ymd_opt(2023, 1, 1).unwrap();
    let mut rng = Rng::new(11);

    (0..365)
        .map(|day| {
            let date = start + Duration::days(day);
            let season = ((day as f64 / 365.) * std::f64::consts::TAU - 1.9).sin();
            let base = 12. + season * 11.;
            let noise = rng.gaussian() * 2.5;
            Table::empty()
                .with("date", Value::date(date))
                .with("temperature", Value::Number(round1(base + noise)))
                .with(
                    "humidity",
                    Value::Number(round1(60. + season * 15. + rng.gaussian() * 4.)),
                )
        })
        .collect()
}

/// Categorical counts, the shape of Plot's bar chart examples.
pub fn sales() -> Vec<Table> {
    const REGIONS: [&str; 5] = ["North", "South", "East", "West", "Central"];
    let mut rng = Rng::new(3);

    REGIONS
        .iter()
        .map(|region| {
            Table::empty()
                .with("region", Value::text(*region))
                .with("sales", Value::Number((rng.next_in(40., 180.)).round()))
        })
        .collect()
}

/// Stacked series over months, the shape of Plot's stacked area example.
pub fn stacked() -> Vec<Table> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut rng = Rng::new(5);

    MONTHS
        .iter()
        .map(|month| {
            Table::empty()
                .with("month", Value::text(*month))
                .with("desktop", Value::Number((rng.next_in(20., 60.)).round()))
                .with("mobile", Value::Number((rng.next_in(30., 90.)).round()))
                .with("tablet", Value::Number((rng.next_in(5., 25.)).round()))
        })
        .collect()
}

/// Long-form stacked data: one row per (month, platform).
pub fn long_stacked() -> Vec<Table> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    const PLATFORMS: [&str; 3] = ["Desktop", "Mobile", "Tablet"];
    let mut rng = Rng::new(9);
    let mut rows = Vec::with_capacity(MONTHS.len() * PLATFORMS.len());

    for month in MONTHS {
        for platform in PLATFORMS {
            rows.push(
                Table::empty()
                    .with("month", Value::text(month))
                    .with("platform", Value::text(platform))
                    .with("visits", Value::Number((rng.next_in(10., 80.)).round())),
            );
        }
    }

    rows
}

/// Two well-separated clouds, for the Delaunay and Voronoi marks: enough
/// points that the triangulation has structure to show.
pub fn clusters() -> Vec<Table> {
    let mut rng = Rng::new(31);
    let centres = [(3.0, 3.0), (8.0, 7.0)];
    let mut rows = Vec::with_capacity(60);

    for (index, (cx, cy)) in centres.into_iter().enumerate() {
        for _ in 0..30 {
            rows.push(
                Table::empty()
                    .with("x", Value::Number(round2(cx + rng.gaussian() * 1.4)))
                    .with("y", Value::Number(round2(cy + rng.gaussian() * 1.4)))
                    .with(
                        "group",
                        Value::text(if index == 0 { "left" } else { "right" }),
                    ),
            );
        }
    }

    rows
}

/// Monthly readings from an uneven set of sensors, for the grouping transforms:
/// several rows per month that a group reduces to one range and one count.
pub fn readings() -> Vec<Table> {
    const MONTHS: [&str; 8] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug"];
    let mut rng = Rng::new(37);
    let mut rows = Vec::new();

    for (month, name) in MONTHS.into_iter().enumerate() {
        let drift = 40. + month as f64 * 4.;
        // Sensors report in uneven numbers, so a count per month has a shape.
        let sensors = 4 + (month * 2) % 5;
        for sensor in 0..sensors {
            rows.push(
                Table::empty()
                    .with("month", Value::text(name))
                    .with("sensor", Value::Number(sensor as f64))
                    .with(
                        "reading",
                        Value::Number(round1(drift + rng.next_in(-14., 14.))),
                    ),
            );
        }
    }

    rows
}

/// Quarterly sales by region and channel, for the facet grid: two categorical
/// fields to split the figure by, and a quantitative one to draw.
pub fn faceted_sales() -> Vec<Table> {
    const REGIONS: [&str; 3] = ["North", "South", "East"];
    const CHANNELS: [&str; 2] = ["Retail", "Online"];
    const QUARTERS: [&str; 4] = ["Q1", "Q2", "Q3", "Q4"];
    let mut rng = Rng::new(29);
    let mut rows = Vec::with_capacity(REGIONS.len() * CHANNELS.len() * QUARTERS.len());

    for region in REGIONS {
        for channel in CHANNELS {
            for quarter in QUARTERS {
                rows.push(
                    Table::empty()
                        .with("region", Value::text(region))
                        .with("channel", Value::text(channel))
                        .with("quarter", Value::text(quarter))
                        .with("sales", Value::Number((rng.next_in(20., 90.)).round())),
                );
            }
        }
    }

    rows
}

/// A wide grid of values over two categorical fields, for a heatmap cell mark.
pub fn heatmap() -> Vec<Table> {
    const COLUMNS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const ROWS: [&str; 8] = ["00", "03", "06", "09", "12", "15", "18", "21"];
    let mut rng = Rng::new(13);
    let mut rows = Vec::with_capacity(ROWS.len() * COLUMNS.len());

    for row in ROWS {
        for column in COLUMNS {
            // A workload curve: busy at midday, quiet at night.
            let hour = row.parse::<f64>().unwrap_or(0.);
            let busy = 1. - ((hour - 13.) / 8.).abs();
            let value = (busy * 40. + rng.next_in(0., 18.)).round();
            rows.push(
                Table::empty()
                    .with("hour", Value::text(row))
                    .with("day", Value::text(column))
                    .with("activity", Value::Number(value)),
            );
        }
    }

    rows
}

/// A two-dimensional cloud with two clusters, for hexbin and density marks.
pub fn cloud() -> Vec<Table> {
    let mut rng = Rng::new(17);
    let mut rows = Vec::with_capacity(400);

    for index in 0..400 {
        let (cx, cy, spread, group) = if index % 3 == 0 {
            (2.0, 6.0, 0.8, "alpha")
        } else {
            (6.0, 2.0, 1.2, "beta")
        };
        rows.push(
            Table::empty()
                .with("x", Value::Number(round2(cx + rng.gaussian() * spread)))
                .with("y", Value::Number(round2(cy + rng.gaussian() * spread)))
                .with("group", Value::text(group)),
        );
    }
    rows
}

/// Grouped distributions, for box and violin marks.
pub fn distributions() -> Vec<Table> {
    const GROUPS: [(&str, f64, f64); 4] = [
        ("control", 50., 6.),
        ("treatment-a", 56., 9.),
        ("treatment-b", 62., 4.),
        ("placebo", 52., 12.),
    ];
    let mut rng = Rng::new(19);
    let mut rows = Vec::with_capacity(GROUPS.len() * 60);

    for (group, mean, spread) in GROUPS {
        for _ in 0..60 {
            rows.push(Table::empty().with("group", Value::text(group)).with(
                "score",
                Value::Number(round1(mean + rng.gaussian() * spread)),
            ));
        }
    }

    rows
}

/// Market shares, for pie, arc and waffle marks.
pub fn market_share() -> Vec<Table> {
    [
        ("Search", 46.),
        ("Social", 24.),
        ("Direct", 18.),
        ("Referral", 8.),
        ("Email", 4.),
    ]
    .into_iter()
    .map(|(channel, share)| {
        Table::empty()
            .with("channel", Value::text(channel))
            .with("share", Value::Number(share))
    })
    .collect()
}

/// A bipartite flow, for the link mark.
pub fn flows() -> Vec<Table> {
    [
        ("Solar", "Homes", 32.),
        ("Solar", "Industry", 18.),
        ("Wind", "Homes", 24.),
        ("Wind", "Industry", 14.),
        ("Hydro", "Homes", 8.),
        ("Hydro", "Industry", 4.),
    ]
    .into_iter()
    .map(|(source, target, value)| {
        Table::empty()
            .with("source", Value::text(source))
            .with("target", Value::text(target))
            .with("value", Value::Number(value))
            .with("source_x", Value::Number(source_index(source)))
            .with("target_x", Value::Number(target_index(target)))
            .with("row", Value::Number(flow_row(source, target)))
    })
    .collect()
}

fn source_index(source: &str) -> f64 {
    match source {
        "Solar" => 0.,
        "Wind" => 1.,
        _ => 2.,
    }
}

fn target_index(target: &str) -> f64 {
    match target {
        "Homes" => 0.,
        _ => 1.,
    }
}

fn flow_row(source: &str, target: &str) -> f64 {
    let source = source_index(source);
    let target = target_index(target);
    source * 2. + target
}

/// A family tree as slash-separated paths, for the `tree` mark: one row per
/// descendant path, so shared prefixes make the hierarchy.
pub fn genealogy() -> Vec<Table> {
    const PATHS: [&str; 13] = [
        "Root",
        "Root/Athena",
        "Root/Athena/Pallas",
        "Root/Athena/Nike",
        "Root/Ares",
        "Root/Ares/Deimos",
        "Root/Ares/Phobos",
        "Root/Hermes",
        "Root/Hermes/Pan",
        "Root/Hermes/Daphnis",
        "Root/Apollo",
        "Root/Apollo/Asclepius",
        "Root/Apollo/Python",
    ];
    PATHS
        .into_iter()
        .map(|path| {
            Table::empty()
                .with("path", Value::text(path))
                .with("node", Value::text(path.rsplit('/').next().unwrap_or(path)))
        })
        .collect()
}

/// A short, irregular series for comparing curve interpolations: a dozen
/// points that make a spline's overshoot visible.
pub fn curve_points() -> Vec<Table> {
    const VALUES: [f64; 12] = [12., 34., 28., 58., 46., 72., 40., 66., 54., 80., 62., 88.];
    VALUES
        .into_iter()
        .enumerate()
        .map(|(index, value)| {
            Table::empty()
                .with("step", Value::Number(index as f64))
                .with("value", Value::Number(value))
        })
        .collect()
}

/// A time series with a moving average, for the window transform.
pub fn noisy_series() -> Vec<Table> {
    let start = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
    let mut rng = Rng::new(23);
    let mut value = 100.;

    (0..120)
        .map(|day| {
            value += rng.gaussian() * 4. + 0.3;
            Table::empty()
                .with(
                    "date",
                    Value::date((start + Duration::days(day)).date_naive()),
                )
                .with("value", Value::Number(round1(value)))
        })
        .collect()
}

fn round1(value: f64) -> f64 {
    (value * 10.).round() / 10.
}

fn round2(value: f64) -> f64 {
    (value * 100.).round() / 100.
}
