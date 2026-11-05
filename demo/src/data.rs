//! The datasets Plot's documentation draws, read from `demo/data/` the way
//! the docs load them: `d3.csv(url, d3.autoType)`.
//!
//! The files are copied from Observable Plot's repository
//! (`docs/public/data/`). Each loader parses its file once per thread and
//! hands out a cheap clone.

use std::cell::RefCell;
use std::collections::HashMap;

use gpui_plot::{Dataset, Table, Value, io};

/// The text of a file in `demo/data/`.
pub fn text(file: &str) -> String {
    let path = format!("{}/data/{file}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

thread_local! {
    static CACHE: RefCell<HashMap<&'static str, Dataset>> = RefCell::new(HashMap::new());
}

/// A dataset parsed once and kept for the thread.
fn cached(key: &'static str, load: impl FnOnce() -> Dataset) -> Dataset {
    if let Some(data) = CACHE.with(|cache| cache.borrow().get(key).cloned()) {
        return data;
    }
    let data = load();
    CACHE.with(|cache| cache.borrow_mut().insert(key, data.clone()));
    data
}

macro_rules! csv_datasets {
    ($($(#[$doc:meta])* $name:ident => $file:literal),* $(,)?) => {
        $(
            $(#[$doc])*
            pub fn $name() -> Dataset {
                cached($file, || {
                    if $file.ends_with(".tsv") {
                        io::tsv(&text($file))
                    } else {
                        io::csv(&text($file))
                    }
                })
            }
        )*
    };
}

csv_datasets! {
    /// Apple's daily stock price, 2013–2018.
    aapl => "aapl.csv",
    /// The relative frequency of English letters.
    alphabet => "alphabet.csv",
    /// Amazon's daily stock price.
    amzn => "amzn.csv",
    /// Anscombe's quartet.
    anscombe => "anscombe.csv",
    /// Athletes of the 2016 Summer Olympics, the docs' `olympians`.
    olympians => "athletes.csv",
    /// Barley yields in Minnesota, 1931–32.
    barley => "barley.csv",
    /// Unemployment by industry.
    bls_industry_unemployment => "bls-industry-unemployment.csv",
    /// Unemployment by metro division.
    bls_metro_unemployment => "bls-metro-unemployment.csv",
    /// An aeromagnetic survey off Caledonia.
    ca55 => "ca55-south.csv",
    /// Caltrain departures.
    caltrain => "caltrain.csv",
    /// Cars of the 1970s and early 80s.
    cars => "cars.csv",
    /// Civilizations' spans.
    civilizations => "civilizations.csv",
    /// Diamonds: carat, cut, color, clarity and price.
    diamonds => "diamonds.csv",
    /// The Dow Jones Industrial Average.
    dji => "dji.csv",
    /// Miles driven and the price of gas, 1956–2010.
    driving => "driving.csv",
    /// The Flare software package's classes, by path.
    flare => "flare.csv",
    /// NASA's global temperature anomaly, monthly.
    gistemp => "gistemp.csv",
    /// Google's daily stock price.
    goog => "goog.csv",
    /// IBM's daily stock price.
    ibm => "ibm.csv",
    /// Income by gender and age.
    income_gender => "income-gender.csv",
    /// IPOs' market value and returns.
    ipos => "ipos.csv",
    /// Metro areas' population and inequality, 1980 and 2015.
    metros => "metros.csv",
    /// Michelson–Morley's speed-of-light measurements.
    morley => "morley.csv",
    /// Palmer penguins.
    penguins => "penguins.csv",
    /// World Bank poverty headcounts.
    povcalnet => "povcalnet.csv",
    /// RIAA's US music revenue by format.
    riaa => "riaa-us-revenue.csv",
    /// Seattle's daily weather.
    seattle => "seattle-weather.csv",
    /// San Francisco and San Jose daily temperatures.
    sf_sj_temperatures => "sf-sj-temperatures.csv",
    /// San Francisco's daily high and low temperatures.
    sftemp => "sf-temperatures.csv",
    /// The Simpsons' episodes and ratings.
    simpsons => "simpsons.csv",
    /// Likert survey responses.
    survey => "survey.csv",
    /// The Tour de France 2017, stage 8's elevation.
    tdf => "tdf-stage-8-2017.csv",
    /// Traffic counts by location and hour.
    traffic => "traffic.csv",
    /// TSA checkpoint travelers.
    travelers => "travelers.csv",
    /// TSA throughput by year.
    tsa => "tsa.csv",
    /// The 118th US Congress's members.
    congress => "us-congress-2023.csv",
    /// US county populations.
    us_county_population => "us-county-population.csv",
    /// US county unemployment rates.
    us_county_unemployment => "us-county-unemployment.csv",
    /// US presidential favorability ratings.
    us_president_favorability => "us-president-favorability.csv",
    /// US presidential election results by county, 2020.
    us_presidential_election_2020 => "us-presidential-election-2020.csv",
    /// US state populations, 2010 and 2019.
    us_state_population => "us-state-population-2010-2019.csv",
    /// Wind speed and direction off the coast of Europe.
    wind => "wind.csv",
    /// Old Faithful's eruptions.
    faithful => "faithful.tsv",
    /// Walmart store openings.
    walmarts => "walmarts.tsv",
    /// A beagle's GPS track, as longitude/latitude pairs.
    beagle_raw => "beagle.csv",
}

/// Deaths in the Crimean War by cause, as the docs reshape it: one row per
/// date and cause (`disease`, `wounds`, `other`).
pub fn crimea() -> Dataset {
    cached("crimea", || {
        let text = text("crimean-war.csv");
        let columns = io::columns(&text, b',');
        let wide = io::csv(&text);
        Dataset::new(columns[2..].iter().flat_map(|cause| {
            wide.rows().iter().map(move |row| {
                Table::empty()
                    .with("date", row.value("date"))
                    .with("cause", Value::text(cause.clone()))
                    .with("deaths", row.value(cause))
            })
        }))
    })
}

/// HadCRUT's annual global temperature anomaly: `year` and `anomaly`.
pub fn hadcrut() -> Dataset {
    cached("hadcrut", || {
        Dataset::new(text("hadcrut-annual.txt").lines().filter_map(|line| {
            let mut fields = line.split_whitespace();
            let year = fields.next()?.parse::<f64>().ok()?;
            let anomaly = fields.next()?.parse::<f64>().ok()?;
            Some(
                Table::empty()
                    .with("year", Value::Number(year))
                    .with("anomaly", Value::Number(anomaly)),
            )
        }))
    })
}

/// The population of each state by age group, reshaped tidy as the docs do:
/// `state`, `age` and `population`, the ages in the file's column order.
pub fn stateage() -> (Dataset, Vec<String>) {
    let text = text("us-population-state-age.csv");
    let ages: Vec<String> = io::columns(&text, b',')[1..].to_vec();
    let wide = io::csv(&text);
    let data = Dataset::new(ages.iter().flat_map(|age| {
        wide.rows().iter().map(move |row| {
            Table::empty()
                .with("state", row.value("name"))
                .with("age", Value::text(age.clone()))
                .with("population", row.value(age))
        })
    }));
    (data, ages)
}

/// The JSON text of a file in `demo/data/`, parsed.
pub fn json(file: &str) -> serde_json::Value {
    serde_json::from_str(&text(file))
        .unwrap_or_else(|error| panic!("invalid JSON in {file}: {error}"))
}

/// A seeded standard-normal generator, standing in for the docs'
/// `d3.randomNormal()`: Plot's examples draw fresh samples on every render,
/// and a seed keeps the port's output stable.
pub fn random_normal(seed: u64) -> impl FnMut() -> f64 {
    let mut state = seed.max(1);
    let mut uniform = move || {
        // xorshift64*.
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    };
    move || {
        let u1 = uniform().max(f64::MIN_POSITIVE);
        let u2 = uniform();
        (-2. * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }
}

/// `n` records holding `field`, each a draw from [`random_normal`]: the
/// docs' `{length: n}` data with `x: d3.randomNormal()`.
pub fn normal_samples(n: usize, field: &str, seed: u64) -> Dataset {
    let mut normal = random_normal(seed);
    Dataset::new((0..n).map(|_| Table::empty().with(field.to_string(), Value::Number(normal()))))
}
