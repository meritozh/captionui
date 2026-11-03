# gpui-plot

A Plot-style visualization library for [GPUI](https://gpui.rs), built on
[GPUI Kit](https://github.com/longbridge/gpui-kit). It follows the model of
[Observable Plot](https://observablehq.github.io/plot/getting-started): data is
a collection of records, marks read *channels* from each record, scales turn
channel values into positions and colors, and transforms reshape the data
between the two. A chart is a `Plot` value that renders as a GPUI element.

The repository has two crates:

| Path | What it is |
| --- | --- |
| `crates/gpui-plot` | The library. |
| `demo` | A gallery window of 18 charts, plus an offscreen snapshot renderer. |

## Quick start

```rust
use gpui_plot::{Plot, Table, Value};

let data = vec![
    Table::empty().with("region", Value::text("North")).with("sales", Value::Number(158.)),
    Table::empty().with("region", Value::text("South")).with("sales", Value::Number(42.)),
];

let plot = Plot::new(data)
    .title("Sales by region")
    .bar_y(|mark| mark.x("region").y("sales").corner_radius(4.))
    .y_zero(true);
```

A `Plot` is an element: give it a height and put it in a view.

```rust
div().h(px(440.)).child(plot)
```

Run the gallery:

```bash
cargo run -p gpui-plot-demo
```

## Chart types

Every mark Plot documents has a builder on `Plot`, and every channel Plot
supports (`x`, `y`, `x1`, `x2`, `y1`, `y2`, `z`, `fill`, `stroke`, `r`, `text`,
`symbol`, `opacity`, `title`) is settable on each.

**Basic marks**

| Mark | Builder | Notes |
| --- | --- | --- |
| dot | `.dot(…)` | radius (`r`), symbol, series color |
| line | `.line(…)` | curves: natural, linear, step, step-after, step-before |
| areaY / areaX | `.area_y(…)` / `.area_x(…)` | interval-aware |
| barY / barX | `.bar_y(…)` / `.bar_x(…)` | rounded corners, stacking |
| rect | `.rect(…)` | spans `x1..x2`, `y1..y2` |
| cell | `.cell(…)` | fills its band; band scales inferred |
| ruleX / ruleY | `.rule_x(…)` / `.rule_y(…)` | reference lines |
| tickX / tickY | `.tick_x(…)` / `.tick_y(…)` | Plot's "barcode" rug, full frame or one band |
| text | `.text(…)` | `dx`/`dy` offsets, unclipped like Plot |
| vector | `.vector(…)` | arrow from `x1,y1` to `x,y` |
| link | `.link(…)` | curved connector |

**Statistical and radial marks**

| Mark | Builder | Notes |
| --- | --- | --- |
| boxX / boxY | `.box_x(…)` / `.box_y(…)` | median, quartiles, whiskers, caps |
| violinX / violinY | `.violin_x(…)` / `.violin_y(…)` | kernel density per group |
| densityX / densityY | `.density_x(…)` / `.density_y(…)` | KDE with filled baseline |
| hexbin | `.hexbin(…)` | 2-D binning, count ramp |
| contour | `.contour(…)` | density rings over a raster |
| pie / arc | `.pie(…)` / `.arc(…)` | shares or explicit angles |
| waffle | `.waffle(…)` | one cell per percent |

## Features

**Scales.** Linear, log, pow, sqrt and symlog transforms (`x_transform`,
`y_transform`); continuous, band and point scale kinds (`x_scale`, `y_scale`,
inferred from the mark when not pinned); pinned domains (`x_domain`,
`y_domain`); zero baselines (`x_zero`, `y_zero`); inset and padding
(`inset`, `padding`). Ticks are d3-style "nice" values, calendar ticks on date
axes, and log axes subdivide a single decade rather than labelling it once.

**Color.** Categorical palettes (`ColorRange::observable10()` — Plot's default,
used automatically when a theme's chart colors are one hue ramp), continuous
ramps, and diverging ramps (`ColorRange::blue_red()`). `Plot::color(field, range)`
colors marks by a quantitative field; a `fill` channel can read a row or compute
a color per datum.

**Transforms.** `group`, `bin` (with count and custom reducers), `stack`
(zero, center, normalize; long and wide data), `window` (mean, sum, median,
min, max, cumulative mean, difference, change, percent change; middle, start
and end anchors), `normalize`, `sort`, `reverse`, and `filter` are available on
`Mark` or as functions in `gpui_plot::transform`.

**Frame.** Titles and subtitles, a legend in five positions (top, bottom, left,
right, none) that lays out as a row or a column and reserves its measured width,
grid lines with optional dashes, x and y axes with per-axis tick counts, and
margins that shrink to fit the labels actually drawn.

**Interaction.** Hover tracking with a nearest-datum tooltip, a crosshair, and
highlighted dots per series; a custom tooltip body via `Plot::tooltip`; and a
draw-in appear animation (GPUI Kit's `PlotAppear`) that runs once per plot.

**Theming.** `PlotTheme` projects GPUI Kit's active theme onto the tokens a
plot needs. It separates the *categorical* palette (which must span hues) from
the *sequential* ramp (which must be one hue), and falls back to Observable10
when a theme's `chart.1..5` are a single ramp, as GPUI Kit's stock themes are.

## Verification

Two layers of checks:

```bash
cargo test --workspace
```

covers the data model, scales, ticks, transforms, stacking and legend
resolution.

```bash
cargo run -p gpui-plot-demo --example snapshots
```

renders all 18 gallery charts through GPUI's headless renderer, writes each to
`demo/snapshots/*.png`, and asserts each painted more than 12 distinct colors.
This is the check that caught the defects unit tests could not: charts that
painted nothing, legends over the title, clipped axis labels, stacked bars that
read pre-stack values, and a categorical palette that was one blue ramp.

## Coverage against Observable Plot

Implemented: every mark type in Plot's mark reference, all channel types,
the linear-family and ordinal scales, the transforms above, color ranges,
legends, axes, grid, tooltips, and hover highlighting.

Not implemented: **faceting** (`fx`/`fy`), **projections** (geo marks),
**annotations** (`tip` mark, `pointer` decorations beyond the tooltip),
**animation** beyond the initial draw-in, and Plot's **legacy Mark
combinators** (`Plot.plot`'s options object — this library uses typed builders
instead). `arc` and `contour` are approximations: `arc` reads explicit angles
but draws no rounded ring caps, and `contour` draws nested density rings rather
than marching-squares isolines.
