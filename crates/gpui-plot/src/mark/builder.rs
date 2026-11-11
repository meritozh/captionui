//! The typed mark builders (`DotMark`, `LineMark`, …): the `mark_builder!` macro
//! that generates each over `Mark`, invoked in one file per renderer family.

mod basic;
mod delaunay;
mod furniture;
mod radial;

pub use basic::*;
pub use delaunay::*;
pub use furniture::*;
pub use radial::*;

/// Generates a typed mark builder over [`Mark`].
macro_rules! mark_builder {
    ($(#[$meta:meta])* $name:ident, $kind:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug)]
        pub struct $name(pub Mark);

        impl $name {
            /// A mark of this kind with default channels and style.
            pub fn new() -> Self {
                Self(Mark::new($kind))
            }

            /// Wrap an already-configured [`Mark`].
            pub fn from_mark(mark: Mark) -> Self {
                Self(mark)
            }

            /// The mark this builder configures.
            pub fn into_mark(self) -> Mark {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        // Reads go through `Deref` so a builder's channels and style can be
        // inspected; the builder methods are inherent and consuming, so a
        // chain returns the same builder type.
        impl std::ops::Deref for $name {
            type Target = Mark;

            fn deref(&self) -> &Mark {
                &self.0
            }
        }

        impl From<$name> for Mark {
            fn from(builder: $name) -> Mark {
                builder.0
            }
        }

        impl From<Mark> for $name {
            fn from(mark: Mark) -> Self {
                Self(mark)
            }
        }

        impl $name {
            /// Give the mark its own data, Plot's `Plot.dot(data, …)`.
            pub fn data(mut self, data: impl Into<crate::table::Dataset>) -> Self {
                self.0 = self.0.data(data);
                self
            }

            /// Facet the mark into columns, Plot's mark-level `fx`.
            pub fn fx(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.fx(channel);
                self
            }

            /// Facet the mark into rows, Plot's mark-level `fy`.
            pub fn fy(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.fy(channel);
                self
            }

            /// Draw this mark alone, Plot's `mark.plot()`.
            pub fn plot(self) -> crate::plot::Plot {
                crate::plot::Plot::plot().mark(self.0)
            }

            /// Set the x channel.
            pub fn x(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.x(channel);
                self
            }

            /// Set the y channel.
            pub fn y(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.y(channel);
                self
            }

            /// Set the x interval's start.
            pub fn x1(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.x1(channel);
                self
            }

            /// Set the x interval's end.
            pub fn x2(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.x2(channel);
                self
            }

            /// Set the y interval's start.
            pub fn y1(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.y1(channel);
                self
            }

            /// Set the y interval's end.
            pub fn y2(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.y2(channel);
                self
            }

            /// Set the series channel, Plot's `z`.
            pub fn z(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.z(channel);
                self
            }

            /// Set the fill.
            pub fn fill(mut self, color: impl Into<ColorChannel>) -> Self {
                self.0 = self.0.fill(color);
                self
            }

            /// Set the stroke.
            pub fn stroke(mut self, color: impl Into<ColorChannel>) -> Self {
                self.0 = self.0.stroke(color);
                self
            }

            /// Set the radius of a dot, in pixels.
            pub fn r(mut self, radius: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.r(radius);
                self
            }

            /// Set a node's hierarchy path, for the `tree` mark.
            pub fn path(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.path(channel);
                self
            }

            /// Set the text of a label.
            pub fn text(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.text(channel);
                self
            }

            /// Set the symbol of a dot.
            pub fn symbol(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.symbol(channel);
                self
            }

            /// Set the opacity.
            pub fn opacity(mut self, opacity: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.opacity(opacity);
                self
            }

            /// Set the opacity of the mark's fills, Plot's `fillOpacity`.
            pub fn fill_opacity(mut self, opacity: f32) -> Self {
                self.0 = self.0.fill_opacity(opacity);
                self
            }

            /// Set the opacity of the mark's strokes, Plot's `strokeOpacity`.
            pub fn stroke_opacity(mut self, opacity: f32) -> Self {
                self.0 = self.0.stroke_opacity(opacity);
                self
            }

            /// Set a datum's tooltip title.
            pub fn tip_title(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.tip_title(channel);
                self
            }

            /// Set the mark's own title.
            pub fn title(mut self, title: impl Into<SharedString>) -> Self {
                self.0 = self.0.title(title);
                self
            }

            /// Set the stroke width.
            pub fn stroke_width(mut self, width: f32) -> Self {
                self.0 = self.0.stroke_width(width);
                self
            }

            /// Set how the mark connects its points.
            pub fn curve(mut self, curve: impl Into<Curve>) -> Self {
                self.0 = self.0.curve(curve);
                self
            }

            /// Round a bar's or rect's corners.
            pub fn corner_radius(mut self, radius: f32) -> Self {
                self.0 = self.0.corner_radius(radius);
                self
            }

            /// Offset a text label horizontally, Plot's `dx`.
            pub fn dx(mut self, dx: f32) -> Self {
                self.0 = self.0.dx(dx);
                self
            }

            /// Offset a text label vertically, Plot's `dy`.
            pub fn dy(mut self, dy: f32) -> Self {
                self.0 = self.0.dy(dy);
                self
            }

            /// Stack the mark's series.
            pub fn stack(mut self, offset: StackOffset) -> Self {
                self.0 = self.0.stack(offset);
                self
            }

            /// Name the field that separates the mark's series.
            pub fn stack_series(mut self, field: impl Into<SharedString>) -> Self {
                self.0 = self.0.stack_series(field);
                self
            }

            /// Name the value fields to stack, one per series.
            pub fn stack_fields<I, S>(mut self, fields: I) -> Self
            where
                I: IntoIterator<Item = S>,
                S: Into<SharedString>,
            {
                self.0 = self.0.stack_fields(fields);
                self
            }

            /// Bin the mark's data.
            pub fn bin(mut self, thresholds: Thresholds) -> Self {
                self.0 = self.0.bin(thresholds);
                self
            }

            /// Add a bin output, Plot's `Plot.bin({fillOpacity: "count"}, …)`.
            pub fn bin_output(
                mut self,
                name: impl Into<SharedString>,
                reducer: GroupReducer,
            ) -> Self {
                self.0 = self.0.bin_output(name, reducer);
                self
            }

            /// See [`Mark::bin_cumulative`].
            pub fn bin_cumulative(mut self, cumulative: i8) -> Self {
                self.0 = self.0.bin_cumulative(cumulative);
                self
            }

            /// See [`Mark::bin_keep_empty`].
            pub fn bin_keep_empty(mut self) -> Self {
                self.0 = self.0.bin_keep_empty();
                self
            }

            /// Clip the mark to the plot area, Plot's `clip: true`.
            pub fn clip(mut self, clip: bool) -> Self {
                self.0 = self.0.clip(clip);
                self
            }

            /// See [`Mark::clip_sphere`].
            pub fn clip_sphere(mut self) -> Self {
                self.0 = self.0.clip_sphere();
                self
            }

            /// See [`Mark::hexbin_output`].
            pub fn hexbin_output(mut self, name: impl Into<SharedString>, reducer: GroupReducer) -> Self {
                self.0 = self.0.hexbin_output(name, reducer);
                self
            }

            /// See [`Mark::hexbin_width`].
            pub fn hexbin_width(mut self, pixels: f32) -> Self {
                self.0 = self.0.hexbin_width(pixels);
                self
            }

            /// See [`Mark::tree_node`].
            pub fn tree_node(mut self, spec: crate::hierarchy::TreeSpec) -> Self {
                self.0 = self.0.tree_node(spec);
                self
            }

            /// See [`Mark::tree_link`].
            pub fn tree_link(mut self, spec: crate::hierarchy::TreeSpec) -> Self {
                self.0 = self.0.tree_link(spec);
                self
            }

            /// See [`Mark::tree_filter`].
            pub fn tree_filter(mut self, field: &str) -> Self {
                self.0 = self.0.tree_filter(field);
                self
            }

            /// See [`Mark::bandwidth`].
            pub fn bandwidth(mut self, pixels: f32) -> Self {
                self.0 = self.0.bandwidth(pixels);
                self
            }

            /// See [`Mark::thresholds`].
            pub fn thresholds(mut self, count: usize) -> Self {
                self.0 = self.0.thresholds(count);
                self
            }

            /// See [`Mark::threshold_values`].
            pub fn threshold_values(mut self, values: impl IntoIterator<Item = f64>) -> Self {
                self.0 = self.0.threshold_values(values);
                self
            }

            /// See [`Mark::weight`].
            pub fn weight(mut self, weight: impl Fn(&crate::table::Table) -> f64 + 'static) -> Self {
                self.0 = self.0.weight(weight);
                self
            }

            /// See [`Mark::grid`].
            pub fn grid(mut self, width: usize, height: usize) -> Self {
                self.0 = self.0.grid(width, height);
                self
            }

            /// See [`Mark::sample`].
            pub fn sample(mut self, f: impl Fn(f64, f64) -> f64 + 'static) -> Self {
                self.0 = self.0.sample(f);
                self
            }

            /// See [`Mark::interpolate`].
            pub fn interpolate(mut self, how: &str) -> Self {
                self.0 = self.0.interpolate(how);
                self
            }

            /// See [`Mark::blur`].
            pub fn blur(mut self, radius: f64) -> Self {
                self.0 = self.0.blur(radius);
                self
            }

            /// See [`Mark::contour_thresholds`].
            pub fn contour_thresholds(mut self, count: usize) -> Self {
                self.0 = self.0.contour_thresholds(count);
                self
            }

            /// See [`Mark::contour_threshold_values`].
            pub fn contour_threshold_values(mut self, values: impl IntoIterator<Item = f64>) -> Self {
                self.0 = self.0.contour_threshold_values(values);
                self
            }

            /// See [`Mark::contour_interval`].
            pub fn contour_interval(mut self, step: f64) -> Self {
                self.0 = self.0.contour_interval(step);
                self
            }

            /// See [`Mark::area_line`], Plot's area `line: true`.
            pub fn area_line(mut self, line: bool) -> Self {
                self.0 = self.0.area_line(line);
                self
            }

            /// See [`Mark::positive_fill`].
            pub fn positive_fill(mut self, color: &str) -> Self {
                self.0 = self.0.positive_fill(color);
                self
            }

            /// See [`Mark::negative_fill`].
            pub fn negative_fill(mut self, color: &str) -> Self {
                self.0 = self.0.negative_fill(color);
                self
            }

            /// See [`Mark::filter`].
            pub fn filter(mut self, keep: impl Fn(&crate::table::Table) -> bool + 'static) -> Self {
                self.0 = self.0.filter(keep);
                self
            }

            /// See [`Mark::text_stroke`].
            pub fn text_stroke(mut self, color: &str) -> Self {
                self.0 = self.0.text_stroke(color);
                self
            }

            /// See [`Mark::text_stroke_width`].
            pub fn text_stroke_width(mut self, width: f32) -> Self {
                self.0 = self.0.text_stroke_width(width);
                self
            }

            /// See [`Mark::geometry`].
            pub fn geometry(mut self, channel: impl IntoChannel) -> Self {
                self.0 = self.0.geometry(channel);
                self
            }

            /// See [`Mark::geo_centroid`].
            pub fn geo_centroid(mut self) -> Self {
                self.0 = self.0.geo_centroid();
                self
            }

            /// See [`Mark::centroid`].
            pub fn centroid(mut self) -> Self {
                self.0 = self.0.centroid();
                self
            }

            /// See [`Mark::marker`].
            pub fn marker(mut self, marker: &str) -> Self {
                self.0 = self.0.marker(marker);
                self
            }

            /// See [`Mark::marker_start`].
            pub fn marker_start(mut self, marker: &str) -> Self {
                self.0 = self.0.marker_start(marker);
                self
            }

            /// See [`Mark::marker_mid`].
            pub fn marker_mid(mut self, marker: &str) -> Self {
                self.0 = self.0.marker_mid(marker);
                self
            }

            /// See [`Mark::marker_end`].
            pub fn marker_end(mut self, marker: &str) -> Self {
                self.0 = self.0.marker_end(marker);
                self
            }

            /// See [`Mark::ticks`].
            pub fn ticks(mut self, count: usize) -> Self {
                self.0 = self.0.ticks(count);
                self
            }

            /// See [`Mark::tick_values`].
            pub fn tick_values(mut self, values: impl IntoIterator<Item = crate::value::Value>) -> Self {
                self.0 = self.0.tick_values(values);
                self
            }

            /// See [`Mark::tick_interval`].
            pub fn tick_interval(mut self, interval: Interval) -> Self {
                self.0 = self.0.tick_interval(interval);
                self
            }

            /// See [`Mark::tick_format`].
            pub fn tick_format(mut self, format: impl Into<crate::options::TickFormat>) -> Self {
                self.0 = self.0.tick_format(format);
                self
            }

            /// See [`Mark::tick_size`].
            pub fn tick_size(mut self, size: f32) -> Self {
                self.0 = self.0.tick_size(size);
                self
            }

            /// See [`Mark::tick_padding`].
            pub fn tick_padding(mut self, padding: f32) -> Self {
                self.0 = self.0.tick_padding(padding);
                self
            }

            /// See [`Mark::tick_rotate`].
            pub fn tick_rotate(mut self, degrees: f32) -> Self {
                self.0 = self.0.tick_rotate(degrees);
                self
            }

            /// See [`Mark::axis_anchor`].
            pub fn axis_anchor(mut self, anchor: crate::options::AxisAnchor) -> Self {
                self.0 = self.0.axis_anchor(anchor);
                self
            }

            /// See [`Mark::axis_label`].
            pub fn axis_label(mut self, label: impl Into<SharedString>) -> Self {
                self.0 = self.0.axis_label(label);
                self
            }

            /// See [`Mark::no_axis_label`].
            pub fn no_axis_label(mut self) -> Self {
                self.0 = self.0.no_axis_label();
                self
            }

            /// See [`Mark::axis_label_anchor`].
            pub fn axis_label_anchor(mut self, anchor: crate::options::LabelAnchor) -> Self {
                self.0 = self.0.axis_label_anchor(anchor);
                self
            }

            /// See [`Mark::axis_label_offset`].
            pub fn axis_label_offset(mut self, offset: f32) -> Self {
                self.0 = self.0.axis_label_offset(offset);
                self
            }

            /// See [`Mark::axis_label_arrow`].
            pub fn axis_label_arrow(mut self, arrow: bool) -> Self {
                self.0 = self.0.axis_label_arrow(arrow);
                self
            }

            /// See [`Mark::color`].
            pub fn color(mut self, color: &str) -> Self {
                self.0 = self.0.color(color);
                self
            }

            /// See [`Mark::axis_line`].
            pub fn axis_line(mut self, line: bool) -> Self {
                self.0 = self.0.axis_line(line);
                self
            }

            /// See [`Mark::stack_y`].
            pub fn stack_y(mut self) -> Self {
                self.0 = self.0.stack_y();
                self
            }

            /// See [`Mark::stack_y1`].
            pub fn stack_y1(mut self) -> Self {
                self.0 = self.0.stack_y1();
                self
            }

            /// See [`Mark::stack_y2`].
            pub fn stack_y2(mut self) -> Self {
                self.0 = self.0.stack_y2();
                self
            }

            /// See [`Mark::stack_x`].
            pub fn stack_x(mut self) -> Self {
                self.0 = self.0.stack_x();
                self
            }

            /// See [`Mark::stack_x1`].
            pub fn stack_x1(mut self) -> Self {
                self.0 = self.0.stack_x1();
                self
            }

            /// See [`Mark::stack_x2`].
            pub fn stack_x2(mut self) -> Self {
                self.0 = self.0.stack_x2();
                self
            }

            /// See [`Mark::stack_offset`].
            pub fn stack_offset(mut self, offset: crate::render::stack::Offset) -> Self {
                self.0 = self.0.stack_offset(offset);
                self
            }

            /// See [`Mark::stack_order`].
            pub fn stack_order(mut self, order: crate::render::stack::StackOrder) -> Self {
                self.0 = self.0.stack_order(order);
                self
            }

            /// See [`Mark::stack_reverse`].
            pub fn stack_reverse(mut self, reverse: bool) -> Self {
                self.0 = self.0.stack_reverse(reverse);
                self
            }

            /// See [`Mark::normalize_x`].
            pub fn normalize_x(mut self, basis: crate::render::stack::NormalizeBasis) -> Self {
                self.0 = self.0.normalize_x(basis);
                self
            }

            /// See [`Mark::normalize_y`].
            pub fn normalize_y(mut self, basis: crate::render::stack::NormalizeBasis) -> Self {
                self.0 = self.0.normalize_y(basis);
                self
            }

            /// See [`Mark::sort_domain`].
            pub fn sort_domain(mut self, scale: &str, by: &str, reverse: bool) -> Self {
                self.0 = self.0.sort_domain(scale, by, reverse);
                self
            }

            /// See [`Mark::sort_domain_by`].
            pub fn sort_domain_by(mut self, sort: DomainSort) -> Self {
                self.0 = self.0.sort_domain_by(sort);
                self
            }

            /// See [`Mark::format`].
            pub fn format(mut self, format: impl Into<crate::options::TickFormat>) -> Self {
                self.0 = self.0.format(format);
                self
            }

            /// See [`Mark::text_anchor`].
            pub fn text_anchor(mut self, anchor: TextAnchor) -> Self {
                self.0 = self.0.text_anchor(anchor);
                self
            }

            /// See [`Mark::line_anchor`].
            pub fn line_anchor(mut self, anchor: LineAnchor) -> Self {
                self.0 = self.0.line_anchor(anchor);
                self
            }

            /// See [`Mark::frame_anchor`].
            pub fn frame_anchor(mut self, anchor: FrameAnchor) -> Self {
                self.0 = self.0.frame_anchor(anchor);
                self
            }

            /// See [`Mark::font_size`].
            pub fn font_size(mut self, size: f32) -> Self {
                self.0 = self.0.font_size(size);
                self
            }

            /// See [`Mark::font_weight`].
            pub fn font_weight(mut self, weight: u16) -> Self {
                self.0 = self.0.font_weight(weight);
                self
            }

            /// See [`Mark::italic`].
            pub fn italic(mut self) -> Self {
                self.0 = self.0.italic();
                self
            }

            /// See [`Mark::line_width`].
            pub fn line_width(mut self, ems: f32) -> Self {
                self.0 = self.0.line_width(ems);
                self
            }

            /// See [`Mark::tip_anchor`].
            pub fn tip_anchor(mut self, anchor: FrameAnchor) -> Self {
                self.0 = self.0.tip_anchor(anchor);
                self
            }

            /// See [`Mark::text_padding`].
            pub fn text_padding(mut self, padding: f32) -> Self {
                self.0 = self.0.text_padding(padding);
                self
            }

            /// See [`Mark::pointer_size`].
            pub fn pointer_size(mut self, size: f32) -> Self {
                self.0 = self.0.pointer_size(size);
                self
            }

            /// See [`Mark::dash`].
            pub fn dash(mut self, dash: impl IntoIterator<Item = f32>) -> Self {
                self.0 = self.0.dash(dash);
                self
            }

            /// See [`Mark::text_rotate`].
            pub fn text_rotate(mut self, degrees: f32) -> Self {
                self.0 = self.0.text_rotate(degrees);
                self
            }

            /// Shrink a rect or bar on every side, Plot's `inset`.
            pub fn inset(mut self, pixels: f32) -> Self {
                self.0 = self.0.inset(pixels);
                self
            }

            /// See [`Mark::mix_blend_mode`].
            pub fn mix_blend_mode(mut self, mode: BlendMode) -> Self {
                self.0 = self.0.mix_blend_mode(mode);
                self
            }

            /// Bucket a date or number into an interval, Plot's `interval`.
            pub fn interval(mut self, interval: Interval) -> Self {
                self.0 = self.0.interval(interval);
                self
            }

            /// Set the image an `image` mark paints, shared across its rows.
            pub fn image(mut self, image: std::sync::Arc<gpui::RenderImage>) -> Self {
                self.0 = self.0.image(image);
                self
            }

            /// See [`Mark::width`].
            pub fn width(mut self, width: f32) -> Self {
                self.0 = self.0.width(width);
                self
            }

            /// See [`Mark::height`].
            pub fn height(mut self, height: f32) -> Self {
                self.0 = self.0.height(height);
                self
            }

            /// See [`Mark::preserve_aspect_ratio`].
            pub fn preserve_aspect_ratio(mut self, value: &str) -> Self {
                self.0 = self.0.preserve_aspect_ratio(value);
                self
            }

            /// Set the equirectangular projection a `geo` mark maps lon/lat
            /// through.
            pub fn projection(mut self, projection: crate::projection::Projection) -> Self {
                self.0 = self.0.projection(projection);
                self
            }

            /// Add a GeoJSON-like ring a geo mark fills as a projected shape.
            pub fn ring(mut self, ring: Vec<(f64, f64)>) -> Self {
                self.0 = self.0.ring(ring);
                self
            }

            /// Add every polygon in a GeoJSON document to a geo mark.
            pub fn geojson(mut self, text: &str) -> Self {
                self.0 = self.0.geojson(text);
                self
            }

            /// Sort the mark's rows by a field.
            pub fn sort(mut self, field: impl Into<SharedString>, descending: bool) -> Self {
                self.0 = self.0.sort(field, descending);
                self
            }

            /// Reverse the mark's row order.
            pub fn reverse(mut self) -> Self {
                self.0 = self.0.reverse();
                self
            }

            /// Reduce the mark's rows.
            pub fn reduce(mut self, reducer: Reducer) -> Self {
                self.0 = self.0.reduce(reducer);
                self
            }

            /// Group the mark's rows by x, Plot's `groupX`.
            pub fn group_x(mut self) -> Self {
                self.0 = self.0.group_x();
                self
            }

            /// Group by x and y together, Plot's `group`.
            pub fn group_xy(mut self) -> Self {
                self.0 = self.0.group_xy();
                self
            }

            /// Group the mark's rows by y, Plot's `groupY`.
            pub fn group_y(mut self) -> Self {
                self.0 = self.0.group_y();
                self
            }

            /// Group the mark's rows by its `z` channel, Plot's `groupZ`.
            pub fn group_z(mut self) -> Self {
                self.0 = self.0.group_z();
                self
            }

            /// Reduce a named channel over the mark's groups.
            pub fn group_output(
                mut self,
                name: impl Into<SharedString>,
                reducer: GroupReducer,
            ) -> Self {
                self.0 = self.0.group_output(name, reducer);
                self
            }

            /// Apply a window transform to the mark's value.
            pub fn window(mut self, k: usize, reducer: WindowReducer, anchor: Anchor) -> Self {
                self.0 = self.0.window(k, reducer, anchor);
                self
            }

            /// Make the mark's window strict, Plot's `strict: true`.
            pub fn window_strict(mut self, strict: bool) -> Self {
                self.0 = self.0.window_strict(strict);
                self
            }

            /// Normalize the mark's value.
            pub fn normalize(mut self) -> Self {
                self.0 = self.0.normalize();
                self
            }

            /// Pack the mark's points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
            pub fn dodge(mut self, dodge: Dodge) -> Self {
                self.0 = self.0.dodge(dodge);
                self
            }

            /// Keep one row per group, Plot's `select`.
            pub fn select(
                mut self,
                selector: Selector,
                group: Option<impl Into<SharedString>>,
            ) -> Self {
                self.0 = self.0.select(selector, group);
                self
            }

            /// Fold the mark's value into a new field, Plot's `mapX`/`mapY`.
            pub fn map(
                mut self,
                input: impl Into<SharedString>,
                output: impl Into<SharedString>,
                reducer: MapReducer,
            ) -> Self {
                self.0 = self.0.map(input, output, reducer);
                self
            }

            /// Restart the mark's fold at each series.
            pub fn map_group(mut self, group: impl Into<SharedString>) -> Self {
                self.0 = self.0.map_group(group);
                self
            }

            /// Map the mark's x channels, Plot's `mapX(mapper)`.
            pub fn map_x(mut self, mapper: Mapper) -> Self {
                self.0 = self.0.map_x(mapper);
                self
            }

            /// Map the mark's y channels, Plot's `mapY(mapper)`.
            pub fn map_y(mut self, mapper: Mapper) -> Self {
                self.0 = self.0.map_y(mapper);
                self
            }

            /// Map one of the mark's channels, Plot's `map({name: mapper})`.
            pub fn map_channel(mut self, name: impl Into<SharedString>, mapper: Mapper) -> Self {
                self.0 = self.0.map_channel(name, mapper);
                self
            }

            /// Move the mark's value by an offset, Plot's `shiftX`/`shiftY`.
            pub fn shift(
                mut self,
                input: impl Into<SharedString>,
                output: impl Into<SharedString>,
                offset: f64,
            ) -> Self {
                self.0 = self.0.shift(input, output, offset);
                self
            }

            /// Draw the rows in a shuffled order, Plot's `shuffle`.
            pub fn shuffle(mut self, seed: u64) -> Self {
                self.0 = self.0.shuffle(seed);
                self
            }

            /// Curve an arrow by `bend` degrees, Plot's `bend`.
            pub fn bend(mut self, degrees: f32) -> Self {
                self.0 = self.0.bend(degrees);
                self
            }

            /// Turn an arrow's bend by the order of its ends, Plot's `sweep`.
            pub fn sweep(mut self, sweep: crate::mark::Sweep) -> Self {
                self.0 = self.0.sweep(sweep);
                self
            }

            /// Set the angle between an arrow's wings, Plot's `headAngle`.
            pub fn head_angle(mut self, degrees: f32) -> Self {
                self.0 = self.0.head_angle(degrees);
                self
            }

            /// Set the length of an arrow's wings, Plot's `headLength`.
            pub fn head_length(mut self, pixels: f32) -> Self {
                self.0 = self.0.head_length(pixels);
                self
            }

            /// Set a spike's or vector's length, Plot's `length`.
            pub fn length(mut self, pixels: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.length(pixels);
                self
            }

            /// Set a vector's rotation, Plot's `rotate`, in degrees clockwise.
            pub fn rotate(mut self, degrees: impl Into<NumberChannel>) -> Self {
                self.0 = self.0.rotate(degrees);
                self
            }

            /// Set how a vector is anchored at its datum, Plot's `anchor`.
            pub fn anchor(mut self, anchor: VectorAnchor) -> Self {
                self.0 = self.0.anchor(anchor);
                self
            }

            /// Set a regression's confidence level; zero draws the line alone.
            pub fn ci(mut self, level: f32) -> Self {
                self.0 = self.0.ci(level);
                self
            }

            /// Set a Bollinger band's window and multiplier.
            pub fn bollinger(mut self, window: usize, k: f32) -> Self {
                self.0 = self.0.bollinger(window, k);
                self
            }

            /// Set a hexgrid's hexagon width, Plot's `binWidth`.
            pub fn bin_width(mut self, pixels: f32) -> Self {
                self.0 = self.0.bin_width(pixels);
                self
            }

            /// Set the value one waffle cell stands for, Plot's `unit`.
            pub fn unit(mut self, unit: f32) -> Self {
                self.0 = self.0.unit(unit);
                self
            }

            /// Set the gap between a waffle's cells, Plot's `gap`.
            pub fn gap(mut self, pixels: f32) -> Self {
                self.0 = self.0.gap(pixels);
                self
            }

            /// Set how many cells a waffle's row holds, Plot's `multiple`.
            pub fn multiple(mut self, cells: usize) -> Self {
                self.0 = self.0.multiple(cells);
                self
            }
        }
    };
}

use mark_builder;
