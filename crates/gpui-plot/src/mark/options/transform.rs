//! Plot's other transforms: domain sort, filter, tree, hexbin, bin, interval,
//! sort, reduce, group, window, normalize, dodge, select, map, shift, shuffle.

use gpui::SharedString;

use crate::mark::{
    Axis, BinConfig, Curve, DomainSort, FrameAnchor, GroupConfig, GroupOutput, Interval, MapConfig,
    MapReducer, MapTarget, Mark, MarkStyle, RowFilter, SelectConfig, ShiftConfig, SortConfig,
};
use crate::style::ColorChannel;
use crate::transform::{
    Anchor, Dodge, GroupReducer, Mapper, Reducer, Selector, Thresholds, WindowReducer,
};

impl Mark {
    /// Plot's mark `sort: {scale: by}`: order `scale`'s domain by the `by`
    /// channel, the largest value per domain value; `"-x"` or `reverse`
    /// sorts descending.
    pub fn sort_domain(mut self, scale: &str, by: &str, reverse: bool) -> Self {
        let (by, negate) = match by.strip_prefix('-') {
            Some(by) => (by, true),
            None => (by, false),
        };
        // Plot's `channelDomain`: `width` and `height` sort descending by
        // default, every other channel ascending.
        let descending = negate != matches!(by, "width" | "height");
        self.domain_sort.push(DomainSort {
            scale: scale.to_string().into(),
            by: by.to_string().into(),
            reduce: GroupReducer::Max,
            reverse: reverse != descending,
            limit: None,
        });
        self
    }

    /// Plot's mark `sort` with an explicit reducer and limit.
    pub fn sort_domain_by(mut self, sort: DomainSort) -> Self {
        self.domain_sort.push(sort);
        self
    }

    /// Plot's `filter` option: draw only the records `keep` accepts.
    pub fn filter(mut self, keep: impl Fn(&crate::table::Table) -> bool + 'static) -> Self {
        self.filter = Some(RowFilter(std::rc::Rc::new(keep)));
        self
    }

    /// Plot's `treeNode` transform: one row per node of the hierarchy the
    /// records' paths describe, at its layout position, with the `node:name`,
    /// `node:path`, `node:internal`, `node:external`, `node:depth` and
    /// `node:height` fields.
    pub fn tree_node(mut self, spec: crate::hierarchy::TreeSpec) -> Self {
        // Plot's treeNode anchors on the tree's side of the frame, so a text
        // reads away from its node.
        if self.style.frame_anchor.is_none() {
            self.style.frame_anchor = Some(if spec.right {
                FrameAnchor::Right
            } else {
                FrameAnchor::Left
            });
        }
        self.tree = Some((spec, false));
        self.channels.x = Some(crate::table::Channel::Field("$tree:x".into()));
        self.channels.y = Some(crate::table::Channel::Field("$tree:y".into()));
        self
    }

    /// Plot's `treeLink` transform: one row per parent → child link.
    pub fn tree_link(mut self, spec: crate::hierarchy::TreeSpec) -> Self {
        self.tree = Some((spec, true));
        self.channels.x1 = Some(crate::table::Channel::Field("$tree:x1".into()));
        self.channels.y1 = Some(crate::table::Channel::Field("$tree:y1".into()));
        self.channels.x2 = Some(crate::table::Channel::Field("$tree:x2".into()));
        self.channels.y2 = Some(crate::table::Channel::Field("$tree:y2".into()));
        self.style.curve = Curve::BumpX;
        // Plot's treeLink defaults: a `#777` stroke, 1.5px at half opacity.
        let defaults = MarkStyle::default();
        if self.channels.stroke.is_none() && self.style.stroke == defaults.stroke {
            self.channels.stroke = Some(ColorChannel::Constant(crate::scale::hsla_from_hex(
                0x777777,
            )));
        }
        if self.style.stroke_width == defaults.stroke_width {
            self.style.stroke_width = 1.5;
        }
        if self.style.stroke_opacity == defaults.stroke_opacity {
            self.style.stroke_opacity = 0.5;
        }
        self
    }

    /// A tree transform's `treeFilter`.
    pub fn tree_filter(mut self, field: &str) -> Self {
        self.tree_filter = Some(field.to_string().into());
        self
    }

    /// Plot's `hexbin` transform: bin the projected points into hexagons
    /// `bin_width` pixels across and reduce each into `outputs` — `"fill"`,
    /// `"stroke"`, `"r"`, `"text"` … A dot drawn through it is a hexagon
    /// unless given another symbol.
    pub fn hexbin_output(mut self, name: impl Into<SharedString>, reducer: GroupReducer) -> Self {
        let (mut outputs, width) = self.hexbin.take().unwrap_or((Vec::new(), 20.));
        outputs.push(GroupOutput::new(name, reducer));
        self.hexbin = Some((outputs, width));
        self
    }

    /// The hexbin transform's `binWidth`.
    pub fn hexbin_width(mut self, pixels: f32) -> Self {
        let (outputs, _) = self.hexbin.take().unwrap_or((Vec::new(), 20.));
        self.hexbin = Some((outputs, pixels));
        self
    }

    /// Bin the mark's data.
    pub fn bin(mut self, thresholds: Thresholds) -> Self {
        let bin = self.bin.take().unwrap_or_default();
        self.bin = Some(BinConfig {
            thresholds,
            edges: [None, None],
            ..bin
        });
        self
    }

    /// Plot's bin `cumulative` option: `1` accumulates each bin with those
    /// before it, `-1` with those after it.
    pub fn bin_cumulative(mut self, cumulative: i8) -> Self {
        let mut bin = self.bin.take().unwrap_or_default();
        bin.cumulative = cumulative.signum();
        self.bin = Some(bin);
        self
    }

    /// Plot's bin `filter: null`: keep empty bins.
    pub fn bin_keep_empty(mut self) -> Self {
        let mut bin = self.bin.take().unwrap_or_default();
        bin.keep_empty = true;
        self.bin = Some(bin);
        self
    }

    /// Add a bin output, Plot's `Plot.bin({fillOpacity: "count"}, …)`: each
    /// bin's rows are reduced into `name` — `"x"`, `"y"`, `"fill"`,
    /// `"fillOpacity"`, `"opacity"` or `"r"`. Bins with the default
    /// thresholds when the mark has not chosen any.
    pub fn bin_output(mut self, name: impl Into<SharedString>, reducer: GroupReducer) -> Self {
        let mut bin = self.bin.take().unwrap_or_default();
        bin.outputs.push(GroupOutput::new(name, reducer));
        self.bin = Some(bin);
        self
    }

    /// Bucket a date or number into an interval, Plot's `interval`.
    ///
    /// A mark with a singular `x` (or `y`) value and an interval replaces it
    /// with an x interval spanning its bucket: a `Day` on a date turns each
    /// datum into `[midnight, next midnight)`, so a `rectY` draws one bar per
    /// day. A `Month`/`Year` bucket the calendar; a `Step(n)` buckets numbers
    /// by a fixed width.
    pub fn interval(mut self, interval: Interval) -> Self {
        self.interval = Some(interval);
        self
    }

    /// Sort the mark's rows.
    pub fn sort(mut self, field: impl Into<SharedString>, descending: bool) -> Self {
        self.sort = Some(SortConfig {
            field: Some(field.into()),
            descending,
            reverse: false,
        });
        self
    }

    /// Reverse the mark's row order, Plot's `reverse`.
    pub fn reverse(mut self) -> Self {
        let mut config = self.sort.unwrap_or_default();
        config.reverse = !config.reverse;
        self.sort = Some(config);
        self
    }

    /// Reduce the mark's rows with a reducer, Plot's `reduce`.
    pub fn reduce(mut self, reducer: Reducer) -> Self {
        self.reduce = Some(reducer);
        self
    }

    /// Group the mark's rows by x and reduce them, Plot's `groupX`.
    ///
    /// Plot's default output is a count, which is what turns a scatter's rows
    /// into a histogram: every x value becomes one row whose y is how many
    /// readings shared it.
    pub fn group_x(mut self) -> Self {
        let outputs = self.group.take().map(|g| g.outputs).unwrap_or_default();
        self.group = Some(GroupConfig {
            axis: Some(Axis::X),
            outputs,
            both: false,
        });
        self
    }

    /// Group the mark's rows by x and y together, Plot's `group`.
    pub fn group_xy(mut self) -> Self {
        let outputs = self.group.take().map(|g| g.outputs).unwrap_or_default();
        self.group = Some(GroupConfig {
            axis: Some(Axis::X),
            outputs,
            both: true,
        });
        self
    }

    /// Group the mark's rows by y and reduce them, Plot's `groupY`.
    pub fn group_y(mut self) -> Self {
        let outputs = self.group.take().map(|g| g.outputs).unwrap_or_default();
        self.group = Some(GroupConfig {
            axis: Some(Axis::Y),
            outputs,
            both: false,
        });
        self
    }

    /// Group the mark's rows by its `z` channel and reduce them, Plot's
    /// `groupZ`. Without a `z` the whole dataset is one group.
    pub fn group_z(mut self) -> Self {
        let outputs = self.group.take().map(|g| g.outputs).unwrap_or_default();
        self.group = Some(GroupConfig {
            axis: None,
            outputs,
            both: false,
        });
        self
    }

    /// Reduce a named channel over the mark's groups, Plot's
    /// `groupX({y: "mean"}, …)`. The reducer reads the channel it writes
    /// unless [`GroupOutput::input`] names another field.
    pub fn group_output(mut self, name: impl Into<SharedString>, reducer: GroupReducer) -> Self {
        let output = GroupOutput::new(name, reducer);
        match &mut self.group {
            Some(config) => config.outputs.push(output),
            None => {
                self.group = Some(GroupConfig {
                    axis: Some(Axis::X),
                    outputs: vec![output],
                    both: false,
                });
            }
        }
        self
    }

    /// Make the mark's window strict, Plot's `strict: true`.
    pub fn window_strict(mut self, strict: bool) -> Self {
        self.window_strict = strict;
        self
    }

    /// Apply a window transform to the mark's value, Plot's `window`.
    pub fn window(mut self, k: usize, reducer: WindowReducer, anchor: Anchor) -> Self {
        self.window = Some((k, reducer, anchor));
        self
    }

    /// Normalize the mark's value across the data, Plot's `normalize`.
    pub fn normalize(mut self) -> Self {
        self.normalize = true;
        self
    }

    /// Pack the mark's points into a beeswarm, Plot's `dodgeX`/`dodgeY`.
    pub fn dodge(mut self, dodge: Dodge) -> Self {
        self.dodge = Some(dodge);
        self
    }

    /// Keep only the first, last or extreme row of each group, Plot's
    /// `select`. A mark that draws one labelled point per series uses it.
    pub fn select(mut self, selector: Selector, group: Option<impl Into<SharedString>>) -> Self {
        let axis = match &selector {
            Selector::Min(field) | Selector::Max(field) if &**field == "x" => Some(Axis::X),
            Selector::Min(field) | Selector::Max(field) if &**field == "y" => Some(Axis::Y),
            _ => None,
        };
        self.select = Some(SelectConfig {
            selector,
            group: group.map(Into::into),
            axis,
        });
        self
    }

    /// Fold the mark's value into a new field, Plot's `mapX`/`mapY`.
    pub fn map(
        mut self,
        input: impl Into<SharedString>,
        output: impl Into<SharedString>,
        reducer: MapReducer,
    ) -> Self {
        self.map = Some(MapConfig {
            input: input.into(),
            output: output.into(),
            reducer,
            group: None,
        });
        self
    }

    /// Restart the mark's fold at each series, Plot's `mapX` with a `z`.
    pub fn map_group(mut self, group: impl Into<SharedString>) -> Self {
        if let Some(map) = &mut self.map {
            map.group = Some(group.into());
        }
        self
    }

    /// Map the mark's x channels, Plot's `mapX(mapper)`, per `z` series.
    pub fn map_x(mut self, mapper: Mapper) -> Self {
        self.maps.push((MapTarget::X, mapper));
        self
    }

    /// Map the mark's y channels, Plot's `mapY(mapper)`, per `z` series.
    pub fn map_y(mut self, mapper: Mapper) -> Self {
        self.maps.push((MapTarget::Y, mapper));
        self
    }

    /// Map one of the mark's channels, Plot's `map({name: mapper})`, per `z`
    /// series.
    pub fn map_channel(mut self, name: impl Into<SharedString>, mapper: Mapper) -> Self {
        self.maps.push((MapTarget::Channel(name.into()), mapper));
        self
    }

    /// Move the mark's value by an offset, Plot's `shiftX`/`shiftY`.
    pub fn shift(
        mut self,
        input: impl Into<SharedString>,
        output: impl Into<SharedString>,
        offset: f64,
    ) -> Self {
        self.shift = Some(ShiftConfig {
            input: input.into(),
            output: output.into(),
            offset,
        });
        self
    }

    /// Draw the rows in a shuffled order, Plot's `shuffle`.
    pub fn shuffle(mut self, seed: u64) -> Self {
        self.shuffle = Some(seed);
        self
    }
}
