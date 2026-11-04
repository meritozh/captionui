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
| `demo` | A gallery window of 44 charts, plus an offscreen snapshot renderer. |

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
| dotX / dotY | `.dot_x(…)` / `.dot_y(…)` | one-dimensional, dots in the frame's middle |
| line | `.line(…)` | curves: natural, linear, step, step-after, step-before |
| lineX / lineY | `.line_x(…)` / `.line_y(…)` | one-dimensional, spread by index |
| areaY / areaX | `.area_y(…)` / `.area_x(…)` | interval-aware |
| barY / barX | `.bar_y(…)` / `.bar_x(…)` | rounded corners, stacking |
| rect | `.rect(…)` | spans `x1..x2`, `y1..y2` |
| rectX / rectY | `.rect_x(…)` / `.rect_y(…)` | one interval grown from zero, the histogram form |
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

**Delaunay marks**

| Mark | Builder | Notes |
| --- | --- | --- |
| delaunayLink | `.delaunay_link(…)` | every triangulation edge as its own path, colored by the row it starts from |
| delaunayMesh | `.delaunay_mesh(…)` | the triangulation as one path, Plot's 0.2 stroke opacity |
| hull | `.hull(…)` | convex outline; filled only when given a `fill` |
| voronoi | `.voronoi(…)` | one cell per datum, clipped to the plot |
| voronoiMesh | `.voronoi_mesh(…)` | the cells' shared edges, each stroked once |

The geometry is built from the data's *projected* points, so the shapes follow
the scales, and it lives in `gpui_plot::delaunay` for callers that want the
triangulation itself.

## Features

**Scales.** Linear, log, pow, sqrt and symlog transforms (`x_transform`,
`y_transform`); continuous, band and point scale kinds (`x_scale`, `y_scale`,
inferred from the mark when not pinned); pinned domains (`x_domain`,
`y_domain`); zero baselines (`x_zero`, `y_zero`); inset and padding
(`inset`, `padding`). Domains can be widened to round values (`x_nice`,
`y_nice`), positions clamped to the frame (`x_clamp`, `y_clamp`), and an axis
reversed (`x_reverse`, `y_reverse`). Ticks are d3-style "nice" values, calendar
ticks on date axes, and log axes subdivide a single decade rather than
labelling it once.

**Color.** Categorical palettes (`ColorRange::observable10()` — Plot's default,
used automatically when a theme's chart colors are one hue ramp), continuous
ramps, and diverging ramps (`ColorRange::blue_red()`). `Plot::color(field, range)`
colors marks by a quantitative field; a `fill` channel can read a row or compute
a color per datum.

**Transforms.** `group`, `groupX`/`groupY`/`groupZ` (a group's outputs are
reducers over its members: count, distinct, sum, mean, min, max, median,
deviation, first, last, and `find`), `bin` (with count and custom reducers),
`stack` (zero, center, normalize; long and wide data), `window` (mean, sum,
median, min, max, cumulative mean, difference, change, percent change; middle,
start and end anchors), `dodge` (`dodge_x`/`dodge_y` beeswarms, with anchor,
radius and padding), `normalize`, `sort`, `reverse`, and `filter` are available
on `Mark` or as functions in `gpui_plot::transform`.

**Frame.** Titles and subtitles, a legend in five positions (top, bottom, left,
right, none) that lays out as a row or a column and reserves its measured width,
grid lines with optional dashes, x and y axes with per-axis tick counts, and
margins that shrink to fit the labels actually drawn.

**Faceting.** `fx` splits the figure into columns and `fy` into rows, Plot's
small multiples: every facet draws the rows whose facet values match it, all
facets share one x and y scale so they compare directly, and transforms that
summarize (stack, bin, window, reduce) summarize within their own facet. The
axes repeat inside each facet along the grid's first column and last row, a
facet with no data keeps its slot but draws no marks, and the facet values label
the columns and rows outside the plot.

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
resolution. Two suites pin the pipeline's invariants so a rearrangement
cannot silently change the charts: `transform_order_tests` fixes the order
the mark transforms compose in (`prepare_mark`), and a purity test asserts
`prepare_facets` is a pure function, so the once-per-frame facets cache
faithfully mirrors a fresh resolve.

```bash
cargo run -p gpui-plot-demo --example snapshots
```

renders all 44 gallery charts through GPUI's headless renderer, writes each to
`demo/snapshots/*.png`, and asserts each painted more than 12 distinct colors.
This is the check that caught the defects unit tests could not: charts that
painted nothing, legends over the title, clipped axis labels, stacked bars that
read pre-stack values, a categorical palette that was one blue ramp, a
regression band painted above its own fit, and a difference chart whose fills
degenerated to zero area.

## Architecture

The library is layered so each step of the Plot model has one home, and the
seams between them stay open for extension:

- **Data.** `Value` (in `value.rs`) is one scalar with a total order — `Null`
  sorts below everything, booleans as false < true, strings lexicographically,
  numbers and dates numerically — so a channel's extent can be computed without
  the caller declaring its type. `Table`/`Dataset` hold the records.
- **Marks.** `Mark` (`mark.rs`) is a pure data spec: a `MarkKind`, its
  `Channels`, and the transform options it carries. It holds no window state,
  so a chart is a cheap value to build and rebuild. `MarkKind::category()`
  returns one `MarkCategory` table that classifies every mark exactly once —
  which renderer family draws it, its band direction, its bin axis, its stack
  orientation, whether it is bivariate, a length mark, furniture, and how a
  one-dimensional mark sits across its other axis. Every predicate and the
  renderer's dispatch derive from that table rather than re-declaring
  `matches!` whitelists, so adding a mark touches exactly one new row.
- **Preparation.** `prepare_mark` (`render/mod.rs`) applies the transforms to
  the mark's rows: the record rewrites (`select`, `map`, `shift`, `shuffle`)
  run on a cloned `Dataset` before any row resolves; then `sort`, `window`,
  `normalize`, `group`, `reduce`, `bin` and `stack` rewrite the resolved rows.
  The order is pinned by `transform_order_tests`, and the resolved facets are
  cached per frame so hover, tooltip and paint all read the same rows.
- **Scales and frame.** `scale.rs` builds position and color scales;
  `frame.rs` resolves the plot's bounds from the figure and the margins, builds
  the axis ticks, and now owns the furniture painters (grid, axis lines and
  labels, facet labels) that reach into a frame's insets. `facet.rs` splits the
  data into `fx`/`fy` cells that share one scale.
- **Rendering.** Each mark draws through the family `MarkCategory` assigned to
  it: one shape per row in `render/basic.rs`, a summarized or arranged shape in
  `render/radial.rs`, and a lattice over the projected points in
  `render/delaunay.rs`.

Two design choices keep this clean:

- **One code path per concept.** The standalone transforms in `transform.rs`
  (`group`, `normalize`, `sort_by`, `filter`, …) are the public,
  compose-able `Dataset → Dataset` API — README's "as functions in
  `gpui_plot::transform`". The row-level reducers in `render/mod.rs`
  (`collapse_groups`, `reduce_rows`, `stack_rows`) are a separate,
  deliberately optimized path over already-resolved `PreparedRow`s; they are
  not dead duplicates of the same thing, and removing either would break the
  boundary.
- **Deriving intervals in one place.** `derive_intervals` (how a bar grows
  from zero to its value, how a cell fills its band) is one authority invoked
  twice — when a row resolves, and again after grouping, because grouping
  replaces the value a bar's length came from. Both calls are required; folding
  them into one breaks grouped bars.

## Coverage against Observable Plot

Implemented: the basic marks, the statistical and radial marks, the one-
dimensional variants (`dotX`/`dotY`, `lineX`/`lineY`, `rectX`/`rectY`), the
furniture marks (`axisX`/`axisY`, `gridX`/`gridY`, `frame`) and `tip`, all
channel types, the linear-family and ordinal scales with `nice`, `clamp`,
`reverse` and pinned ranges, the transforms above (including `dodge`, `select`,
`map`, `shift`, `shuffle` and `groupX`/`groupY`/`groupZ` with `find`),
faceting (`fx`/`fy`), color ranges, legends
(color, size and opacity, with a title and per-mark swatch shapes), axes, grid,
tooltips, and hover highlighting.

The full mark set now also covers `linearRegressionX`/`Y` with confidence bands,
`bollingerX`/`Y`, `differenceX`/`Y`, `raster`, `hexgrid` and `hexbin` on Plot's
own lattice, `cellX`/`cellY`, `textX`/`textY`, `vectorX`/`vectorY`, `vector`,
`spike`, `arrow`, `waffleX`/`waffleY`, `boxX`/`boxY`, and the Delaunay family
(`delaunayLink`, `delaunayMesh`, `hull`, `voronoi`, `voronoiMesh`).

Not implemented yet:

1. **Geo projections**, which are out of scope for a desktop chart library
   with no basemap ecosystem.

### Adding a mark

A new mark kind is classified exactly once in `MarkKind::category()` and
drawn by one arm in its renderer family (`render/basic.rs`,
`render/radial.rs` or `render/delaunay.rs`). The compiler's exhaustive
match over `MarkKind` — in `category()` and the renderers — turns a
forgotten kind into a compile error rather than silent misrendering. Every
predicate (`is_bivariate`, `is_x_only`), the band-and-bin decisions and the
paint-family dispatch read the same `MarkCategory` table, so adding a mark
means: one enum variant, one `name`, one `category()` row, one draw arm.

The `tension` option for cardinal curves is also missing; the curves
themselves (`basis`, `cardinal`, `catmull-rom`, `monotone`, `bump`, `bundle`,
each with its open and closed variants) are implemented. `raster` lacks
`interpolate`/`width`/`height`, and there is no `image` mark that reads pixels
from a `src` channel.

`arc` and `contour` are approximations: `arc` reads explicit angles but draws
no rounded ring caps, and `contour` draws nested density rings rather than
marching-squares isolines.
