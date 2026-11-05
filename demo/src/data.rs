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

thread_local! {
    static TOPOLOGIES: RefCell<HashMap<&'static str, std::rc::Rc<gpui_plot::Topology>>> = RefCell::new(HashMap::new());
}

/// A TopoJSON file in `demo/data/`, decoded once per thread.
pub fn topology(file: &'static str) -> std::rc::Rc<gpui_plot::Topology> {
    if let Some(t) = TOPOLOGIES.with(|cache| cache.borrow().get(file).cloned()) {
        return t;
    }
    let t = std::rc::Rc::new(gpui_plot::Topology::parse(&json(file)).expect("a TopoJSON topology"));
    TOPOLOGIES.with(|cache| cache.borrow_mut().insert(file, t.clone()));
    t
}

/// The docs' `us`: us-counties-10m.json.
pub fn us() -> std::rc::Rc<gpui_plot::Topology> {
    topology("us-counties-10m.json")
}

/// The docs' `world`: countries-110m.json.
pub fn world() -> std::rc::Rc<gpui_plot::Topology> {
    topology("countries-110m.json")
}

/// `topojson.mesh(us, us.objects.states, (a, b) => a !== b)`: the borders
/// between states.
pub fn statemesh() -> gpui_plot::Geometry {
    us().mesh("states", Some(&|a, b| a != b))
}

/// `topojson.mesh(us, us.objects.nation)`, the nation's outline.
pub fn nation() -> gpui_plot::Geometry {
    us().mesh("nation", None)
}

/// `topojson.feature(us, us.objects.counties).features`, each with the
/// docs' `population` joined from us-county-population.csv by FIPS code.
pub fn counties() -> Dataset {
    cached("counties", || {
        let population: HashMap<String, f64> = us_county_population()
            .rows()
            .iter()
            .filter_map(|row| {
                // The docs key by state + county as text: "01" + "001".
                let code = |field: &str| match row.value(field) {
                    Value::Number(n) => format!(
                        "{:0width$}",
                        n as i64,
                        width = if field == "state" { 2 } else { 3 }
                    ),
                    other => other.label().to_string(),
                };
                Some((
                    format!("{}{}", code("state"), code("county")),
                    row.number("population")?,
                ))
            })
            .collect();
        Dataset::new(us().feature("counties").rows().iter().map(|row| {
            let id = row.value("id").label().to_string();
            match population.get(&id) {
                Some(p) => row.clone().with("population", Value::Number(*p)),
                None => row.clone(),
            }
        }))
    })
}

/// `topojson.feature(us, us.objects.states).features`.
pub fn states() -> Dataset {
    cached("states", || us().feature("states"))
}

/// `topojson.feature(world, world.objects.land)`.
pub fn land() -> gpui_plot::Geometry {
    world()
        .feature_geometry("land")
        .unwrap_or(gpui_plot::Geometry::Collection(Vec::new()))
}

/// `topojson.feature(world, world.objects.countries).features`.
pub fn countries() -> Dataset {
    cached("countries", || world().feature("countries"))
}

/// A FIPS code as d3 reads it from a CSV without `autoType`: text, zero
/// padded to `width` digits (`d3.autoType` would have made it a number).
pub fn fips(value: &Value, width: usize) -> String {
    match value {
        Value::Number(n) => format!("{:0width$}", *n as i64),
        other => other.label().to_string(),
    }
}

/// The geo page's `counties`: `topojson.feature(us, us.objects.counties)`
/// with each county's `unemployment` rate joined by id from
/// us-county-unemployment.csv.
pub fn counties_unemployment() -> Dataset {
    cached("counties_unemployment", || {
        let rate: HashMap<String, f64> = us_county_unemployment()
            .rows()
            .iter()
            .filter_map(|row| Some((fips(&row.value("id"), 5), row.number("rate")?)))
            .collect();
        Dataset::new(us().feature("counties").rows().iter().map(|row| {
            let id = row.value("id").label().to_string();
            let unemployment = rate.get(&id).map_or(Value::Null, |r| Value::Number(*r));
            row.clone().with("unemployment", unemployment)
        }))
    })
}

/// The vector page's `counties`: [`counties`] (with `population`) and each
/// county's `margin2020` and `votes` joined by FIPS code from
/// us-presidential-election-2020.csv.
pub fn counties_election() -> Dataset {
    cached("counties_election", || {
        let election: HashMap<String, (Value, Value)> = us_presidential_election_2020()
            .rows()
            .iter()
            .map(|row| {
                (
                    fips(&row.value("fips"), 5),
                    (row.value("margin2020"), row.value("votes")),
                )
            })
            .collect();
        Dataset::new(counties().rows().iter().map(|row| {
            let id = row.value("id").label().to_string();
            match election.get(&id) {
                Some((margin, votes)) => row
                    .clone()
                    .with("margin2020", margin.clone())
                    .with("votes", votes.clone()),
                None => row.clone(),
            }
        }))
    })
}

/// The rect page's `countyboxes`: `counties.map((d) => d3.geoBounds(d).flat())`,
/// each county's `[west, south, east, north]` as fields "0" to "3". The
/// bounds are the extent of the county's vertices, which is d3's answer for
/// shapes this small that do not cross the antimeridian.
pub fn countyboxes() -> Dataset {
    fn extend(geometry: &gpui_plot::Geometry, b: &mut [f64; 4]) {
        use gpui_plot::Geometry::*;
        let mut point = |[x, y]: [f64; 2]| {
            *b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        };
        match geometry {
            Point(p) => point(*p),
            MultiPoint(ps) | LineString(ps) => ps.iter().copied().for_each(point),
            MultiLineString(rings) | Polygon(rings) => rings.iter().flatten().copied().for_each(point),
            MultiPolygon(polygons) => polygons.iter().flatten().flatten().copied().for_each(point),
            Collection(items) => items.iter().for_each(|g| extend(g, b)),
            Sphere => {}
        }
    }
    cached("countyboxes", || {
        Dataset::new(us().feature("counties").rows().iter().filter_map(|row| {
            let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
            extend(row.value("geometry").as_geo()?, &mut b);
            b[0].is_finite().then(|| {
                Table::empty()
                    .with("0", b[0])
                    .with("1", b[1])
                    .with("2", b[2])
                    .with("3", b[3])
            })
        }))
    })
}

/// The geo page's `walmarts`: each store a Point feature, its opening
/// `date` a property.
pub fn walmart_points() -> Dataset {
    cached("walmart_points", || {
        Dataset::new(walmarts().rows().iter().filter_map(|row| {
            let point =
                gpui_plot::Geometry::Point([row.number("longitude")?, row.number("latitude")?]);
            Some(
                Table::empty()
                    .with("geometry", point.into_value())
                    .with("date", row.value("date")),
            )
        }))
    })
}
