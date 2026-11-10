//! Faceting: Plot's `fx`/`fy` options, which split one figure into a grid of
//! small multiples.
//!
//! Plot lays the facets out with two band scales over the plot — one across
//! it for `fx`, one down it for `fy` — and gives every facet the same plot
//! size, so the x and y scales stay shared. A facet draws only the rows whose
//! `fx`/`fy` values it stands for, and a transform that summarizes data (a
//! stack, a bin) then summarizes within its facet rather than across the
//! whole figure.

use gpui::{Bounds, Pixels, Point, SharedString, point, px};

use crate::value::{Value, ValueKey};

/// The gap between a facet's plot and the label on its facet axis.
pub const FACET_LABEL_GAP: f32 = 6.;

/// Plot's inner padding between facets, as a fraction of a band's step.
const FACET_PADDING: f32 = 0.1;

/// The values facet channels read from each row of their data, kept by
/// (dataset, channel) so the grid and the facet partitions read each pair
/// once. Each value is the channel's [`read`](crate::table::Channel::read)
/// of that row, which is all a partition reads.
#[derive(Default)]
pub(crate) struct FacetReads(Vec<(crate::table::Dataset, crate::table::Channel, Vec<Value>)>);

impl FacetReads {
    /// The values `channel` read from each row of `data`, if they were read.
    pub(crate) fn get(
        &self,
        data: &crate::table::Dataset,
        channel: &crate::table::Channel,
    ) -> Option<&[Value]> {
        self.0
            .iter()
            .find(|(d, c, _)| d.same_rows(data) && c.same(channel))
            .map(|(_, _, values)| values.as_slice())
    }

    /// The values `channel` reads from each row of `data`, read now unless
    /// they already were.
    pub(crate) fn read(
        &mut self,
        data: &crate::table::Dataset,
        channel: &crate::table::Channel,
    ) -> &[Value] {
        let index = match self
            .0
            .iter()
            .position(|(d, c, _)| d.same_rows(data) && c.same(channel))
        {
            Some(index) => index,
            None => {
                let values = data.rows().iter().map(|row| channel.read(row)).collect();
                self.0.push((data.clone(), channel.clone(), values));
                self.0.len() - 1
            }
        };
        &self.0[index].2
    }
}

/// One facet of the grid.
///
/// A cell with no key on a channel stands for every value of that channel,
/// which is how a figure faceted only by `fx` keeps one row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The `fx` value this facet stands for.
    pub x: Option<ValueKey>,
    /// The `fy` value this facet stands for.
    pub y: Option<ValueKey>,
}

/// The facet grid a plot's `fx`/`fy` options resolve.
///
/// The grid holds the two facet domains, not their pixel layout: the facets
/// cannot be laid out until the frame has resolved the plot's size, which
/// itself depends on how wide the facet labels are.
#[derive(Clone, Debug)]
pub struct Grid {
    /// The `fx` domain: one facet per value, laid out across the plot.
    pub fx: Option<Vec<ValueKey>>,
    /// The `fy` domain: one facet per value, laid out down the plot.
    pub fy: Option<Vec<ValueKey>>,
    /// The facets, in row-major order: `fx` varies fastest, so a run of cells
    /// sharing an `fy` value is one row of the grid.
    pub cells: Vec<Cell>,
    /// Whether each facet has no data; see [`Grid::set_empty`].
    empty: Vec<bool>,
}

impl Grid {
    /// The cross product of the two facet domains, Plot's `createFacets`.
    ///
    /// A plot with no facet channels has one cell with no keys, which draws
    /// the whole data: faceting is then the same code path as a 1×1 grid.
    pub fn new(fx: Option<Vec<ValueKey>>, fy: Option<Vec<ValueKey>>) -> Self {
        let cells = match (&fx, &fy) {
            (Some(fx), Some(fy)) => fy
                .iter()
                .flat_map(|y| {
                    fx.iter().map(move |x| Cell {
                        x: Some(x.clone()),
                        y: Some(y.clone()),
                    })
                })
                .collect(),
            (Some(fx), None) => fx
                .iter()
                .map(|x| Cell {
                    x: Some(x.clone()),
                    y: None,
                })
                .collect(),
            (None, Some(fy)) => fy
                .iter()
                .map(|y| Cell {
                    x: None,
                    y: Some(y.clone()),
                })
                .collect(),
            (None, None) => vec![Cell { x: None, y: None }],
        };
        let empty = vec![false; cells.len()];
        Self {
            fx,
            fy,
            cells,
            empty,
        }
    }

    /// A grid over the distinct `fx`/`fy` values of `values`, or an unfaceted
    /// one when neither channel has any.
    pub fn from_values(fx: Vec<Value>, fy: Vec<Value>) -> Self {
        let domain = |values: Vec<Value>| {
            // Plot sorts an ordinal domain in natural ascending order, so
            // `fy: "sex"` stacks female above male whatever the data order.
            let mut keys = crate::scale::dedup(values.into_iter().map(|value| value.key()));
            keys.sort();
            // Plot keeps a facet for missing values, after the others.
            if let Some(i) = keys.iter().position(|k| k.to_value().is_null()) {
                let null = keys.remove(i);
                keys.push(null);
            }
            (!keys.is_empty()).then_some(keys)
        };
        Self::new(domain(fx), domain(fy))
    }

    /// Whether the plot is faceted at all.
    pub fn is_faceted(&self) -> bool {
        self.fx.is_some() || self.fy.is_some()
    }

    /// How many facets sit side by side.
    pub fn columns(&self) -> usize {
        self.fx.as_ref().map_or(1, |fx| fx.len()).max(1)
    }

    /// How many rows of facets there are.
    pub fn rows(&self) -> usize {
        (self.cells.len() / self.columns()).max(1)
    }

    /// The column of a facet, from the left.
    pub fn column(&self, cell: usize) -> usize {
        cell % self.columns()
    }

    /// The row of a facet, from the top.
    pub fn row(&self, cell: usize) -> usize {
        cell / self.columns()
    }

    /// The width of one facet's plot within a grid `plot_width` pixels wide.
    pub fn facet_width(&self, plot_width: f32) -> f32 {
        match &self.fx {
            Some(fx) => band(0, fx.len(), plot_width).1,
            None => plot_width,
        }
    }

    /// The height of one facet's plot within a grid `plot_height` tall.
    pub fn facet_height(&self, plot_height: f32) -> f32 {
        match &self.fy {
            Some(fy) => band(0, fy.len(), plot_height).1,
            None => plot_height,
        }
    }

    /// The band one facet takes out of the grid's `grid` bounds, in figure
    /// coordinates: the cell's slot, before its axes and labels are reserved.
    pub fn facet(&self, grid: Bounds<Pixels>, cell: usize) -> Bounds<Pixels> {
        let (width, height) = (grid.size.width.as_f32(), grid.size.height.as_f32());
        let start =
            |axis: &Option<Vec<ValueKey>>, key: Option<&ValueKey>, length: f32| match (axis, key) {
                (Some(domain), Some(key)) => domain
                    .iter()
                    .position(|value| value == key)
                    .map(|index| band(index, domain.len(), length).0)
                    .unwrap_or(0.),
                _ => 0.,
            };
        let cell_keys = self.cells.get(cell);
        let x = start(&self.fx, cell_keys.and_then(|cell| cell.x.as_ref()), width);
        let y = start(&self.fy, cell_keys.and_then(|cell| cell.y.as_ref()), height);
        Bounds::new(
            point(grid.origin.x + px(x), grid.origin.y + px(y)),
            gpui::size(px(self.facet_width(width)), px(self.facet_height(height))),
        )
    }

    /// The offset of a facet's band from the grid's origin.
    pub fn offset(&self, grid: Bounds<Pixels>, cell: usize) -> Point<Pixels> {
        self.facet(grid, cell).origin - grid.origin
    }

    /// The label each `fx` value shows on the top of its column.
    pub fn fx_labels(&self) -> Vec<SharedString> {
        self.fx
            .as_ref()
            .map(|fx| fx.iter().map(|key| key.to_value().label()).collect())
            .unwrap_or_default()
    }

    /// The label each `fy` value shows on the right of its row.
    pub fn fy_labels(&self) -> Vec<SharedString> {
        self.fy
            .as_ref()
            .map(|fy| fy.iter().map(|key| key.to_value().label()).collect())
            .unwrap_or_default()
    }

    /// Mark the facets that have no data, from the rows that landed in each.
    ///
    /// Plot only calls a facet empty when *some* facets have data and others
    /// do not: a figure none of whose marks are faceted draws every facet, so
    /// a grid where every facet is empty stays as it is.
    pub fn set_empty(&mut self, populated: &[bool]) {
        let count = populated.iter().filter(|populated| **populated).count();
        self.empty = if count > 0 && count < self.cells.len() {
            populated.iter().map(|populated| !populated).collect()
        } else {
            vec![false; self.cells.len()]
        };
    }

    /// Whether a facet has no data.
    pub fn is_empty(&self, cell: usize) -> bool {
        self.empty.get(cell).copied().unwrap_or(false)
    }

    /// The facet a pair of keys stands for.
    pub fn cell(&self, x: Option<&ValueKey>, y: Option<&ValueKey>) -> Option<usize> {
        self.cells
            .iter()
            .position(|cell| cell.x.as_ref() == x && cell.y.as_ref() == y)
    }

    /// Split `data` into the rows of each facet by facet channels, in the
    /// grid's order. A missing channel matches every facet along its axis,
    /// so a mark faceted by `fy` alone repeats across the `fx` columns.
    pub fn partition_channels(
        &self,
        data: &crate::table::Dataset,
        fx: Option<&crate::table::Channel>,
        fy: Option<&crate::table::Channel>,
    ) -> Vec<Vec<crate::table::Table>> {
        self.partition_reading(data, fx, fy, &FacetReads::default())
    }

    /// [`Grid::partition_channels`], taking a channel's values from `reads`
    /// when building the grid already read it over `data`.
    pub(crate) fn partition_reading(
        &self,
        data: &crate::table::Dataset,
        fx: Option<&crate::table::Channel>,
        fy: Option<&crate::table::Channel>,
        reads: &FacetReads,
    ) -> Vec<Vec<crate::table::Table>> {
        let mut groups: Vec<Vec<crate::table::Table>> = vec![Vec::new(); self.cells.len()];
        // The facets each pair of keys lands in, in the grid's order. An axis
        // without a channel is a wildcard (`None`), matching every facet
        // along it; with one, only facets keyed on that axis can match.
        type CellKey = (Option<ValueKey>, Option<ValueKey>);
        let mut lookup: rustc_hash::FxHashMap<CellKey, Vec<usize>> = Default::default();
        for (index, cell) in self.cells.iter().enumerate() {
            let side =
                |channel: Option<&crate::table::Channel>, have: &Option<ValueKey>| match channel {
                    None => Some(None),
                    Some(_) => have.clone().map(Some),
                };
            if let (Some(x), Some(y)) = (side(fx, &cell.x), side(fy, &cell.y)) {
                lookup.entry((x, y)).or_default().push(index);
            }
        }
        // Each channel with the values it read when the grid was built.
        let fx = fx.map(|channel| (channel, reads.get(data, channel)));
        let fy = fy.map(|channel| (channel, reads.get(data, channel)));
        for (index, row) in data.rows().iter().enumerate() {
            let key = |side: Option<(&crate::table::Channel, Option<&[Value]>)>| {
                side.map(|(channel, values)| match values {
                    Some(values) => values[index].key(),
                    None => channel.read(row).key(),
                })
            };
            if let Some(cells) = lookup.get(&(key(fx), key(fy))) {
                for &cell in cells {
                    groups[cell].push(row.clone());
                }
            }
        }
        groups
    }

    /// Split `data` into the rows of each facet, in the grid's order.
    ///
    /// A row whose `fx`/`fy` values are outside the domains lands in no
    /// facet, the way Plot's facet filter drops it. The second half of the
    /// result says which facets hold rows.
    pub fn partition(
        &self,
        data: &crate::table::Dataset,
        fx: Option<&str>,
        fy: Option<&str>,
    ) -> (Vec<Vec<crate::table::Table>>, Vec<bool>) {
        let mut groups: Vec<Vec<crate::table::Table>> = vec![Vec::new(); self.cells.len()];
        for row in data.rows() {
            let key = |field: Option<&str>| field.map(|field| row.value(field).key());
            let x = key(fx);
            let y = key(fy);
            if let Some(cell) = self.cell(x.as_ref(), y.as_ref()) {
                groups[cell].push(row.clone());
            }
        }
        let populated = groups.iter().map(|rows| !rows.is_empty()).collect();
        (groups, populated)
    }
}

/// The start and width of band `index` of `count` across `length` pixels,
/// following Plot's facet band scale: an inner gap of `0.1` of a step and no
/// outer padding, so the first and last facets meet the plot's edges.
pub fn band(index: usize, count: usize, length: f32) -> (f32, f32) {
    (band_start(index, count, length), band_length(count, length))
}

/// The width (or height) of one band of `count` across `length` pixels.
pub fn band_length(count: usize, length: f32) -> f32 {
    if count <= 1 {
        // A lone facet takes the whole plot: Plot's band scale lays one band
        // out across the range without padding.
        return length;
    }
    (length / count as f32) * (1. - FACET_PADDING)
}

/// Where band `index` of `count` starts across `length` pixels.
fn band_start(index: usize, count: usize, length: f32) -> f32 {
    if count <= 1 {
        return 0.;
    }
    let step = length / count as f32;
    let ratio = 1. + FACET_PADDING / (count - 1) as f32;
    index as f32 * step * ratio
}

#[cfg(test)]
mod tests {
    use super::*;

    fn domain(values: &[&str]) -> Vec<Value> {
        values.iter().map(|value| Value::text(*value)).collect()
    }

    #[test]
    fn facets_are_the_cross_product_of_the_domains() {
        let grid = Grid::from_values(domain(&["a", "b"]), domain(&["1", "2"]));
        assert_eq!(grid.cells.len(), 4);
        assert_eq!(grid.columns(), 2);
        assert_eq!(grid.rows(), 2);
        // Row-major: the first row is both columns of the first `fy` value.
        assert_eq!(grid.cells[0].x, Some(Value::text("a").key()));
        assert_eq!(grid.cells[0].y, Some(Value::text("1").key()));
        assert_eq!(grid.cells[1].x, Some(Value::text("b").key()));
        assert_eq!(grid.cells[2].x, Some(Value::text("a").key()));
        assert_eq!(grid.cells[2].y, Some(Value::text("2").key()));
    }

    #[test]
    fn one_facet_channel_makes_one_row_or_column() {
        let grid = Grid::from_values(domain(&["a", "b", "c"]), Vec::new());
        assert_eq!(grid.cells.len(), 3);
        assert_eq!(grid.rows(), 1);
        assert_eq!(grid.columns(), 3);
        assert_eq!(grid.facet_width(300.), 90.);
        assert_eq!(grid.facet_height(200.), 200.);
    }

    #[test]
    fn facet_bands_leave_a_gap_and_meet_the_edges() {
        let grid = Grid::from_values(domain(&["a", "b"]), Vec::new());
        let plot = Bounds::new(point(px(0.), px(0.)), gpui::size(px(400.), px(200.)));
        assert_eq!(grid.facet(plot, 0).origin.x.as_f32(), 0.);
        let second = grid.facet(plot, 1).origin.x.as_f32();
        // The second band ends exactly at the plot's edge.
        assert_eq!(second + grid.facet_width(400.), 400.);
    }

    #[test]
    fn facets_offset_by_their_band() {
        let grid = Grid::from_values(domain(&["a", "b"]), domain(&["1", "2"]));
        let plot = Bounds::new(point(px(10.), px(20.)), gpui::size(px(400.), px(300.)));
        let third = grid.facet(plot, 2);
        assert_eq!(third.origin.x.as_f32(), 10.);
        // The second row starts after the first band and its gap.
        let height = grid.facet_height(300.);
        let start = 300. - height;
        assert_eq!(third.origin.y.as_f32(), 20. + start);
        assert_eq!(start + height, 300.);
    }

    #[test]
    fn an_unfaceted_grid_is_one_cell() {
        let grid = Grid::from_values(Vec::new(), Vec::new());
        assert!(!grid.is_faceted());
        assert_eq!(grid.cells.len(), 1);
        assert_eq!(grid.facet_width(400.), 400.);
        assert_eq!(grid.cell(None, None), Some(0));
    }

    #[test]
    fn empty_facets_are_only_the_ones_without_data() {
        let mut grid = Grid::from_values(domain(&["a", "b"]), domain(&["1", "2"]));
        grid.set_empty(&[true, true, true, false]);
        assert!(grid.is_empty(3));
        assert!(!grid.is_empty(0));
        // When every facet has data, or none does, no facet reads as empty.
        grid.set_empty(&[true; 4]);
        assert!(!grid.is_empty(3));
    }

    #[test]
    fn facet_reads_are_kept_by_dataset_and_channel() {
        use crate::table::{Channel, Dataset, Table};
        let rows = (0..5).map(|i| Table::new([("v", Value::Number(i as f64))]));
        let data = Dataset::new(rows.clone());
        // The same records in another dataset are read on their own.
        let other = Dataset::new(rows);
        let calls = std::rc::Rc::new(std::cell::Cell::new(0));
        let counted = calls.clone();
        let accessor = Channel::Accessor(std::rc::Rc::new(move |row: &Table| {
            counted.set(counted.get() + 1);
            row.value("v")
        }));
        let field = Channel::from("v");
        let mut reads = FacetReads::default();
        assert!(reads.get(&data, &accessor).is_none());
        let read: Vec<Value> = reads.read(&data, &accessor).to_vec();
        assert_eq!(
            read,
            data.rows()
                .iter()
                .map(|r| accessor.read(r))
                .collect::<Vec<_>>()
        );
        assert_eq!(calls.get(), 10);
        reads.read(&data, &accessor);
        assert_eq!(calls.get(), 10, "a pair already read is not read again");
        assert!(reads.get(&other, &accessor).is_none());
        assert!(reads.get(&data, &field).is_none());
        assert_eq!(
            reads.get(&data.clone(), &accessor.clone()),
            Some(read.as_slice())
        );
    }

    #[test]
    fn partitions_match_a_cell_by_cell_scan() {
        use crate::table::{Channel, Dataset, Table};
        // Keys that only a careful lookup keeps apart or together: null, -0
        // and +0, a NaN an accessor keeps, a boolean, and keys outside the
        // grid's domains.
        let keys = [
            Value::text("a"),
            Value::Null,
            Value::Number(-0.),
            Value::Number(0.),
            Value::Number(f64::NAN),
            Value::Bool(true),
            Value::text("off-grid"),
            Value::text("b"),
        ];
        let data = Dataset::new((0..64).map(|i| {
            Table::new([
                ("x", keys[i % keys.len()].clone()),
                ("y", keys[(i / 3) % keys.len()].clone()),
                ("n", Value::Number(i as f64)),
            ])
        }));
        let fx = Channel::Accessor(std::rc::Rc::new(|row: &Table| row.value("x")));
        let fy = Channel::Accessor(std::rc::Rc::new(|row: &Table| row.value("y")));
        let domain = |values: &[Value]| Some(values.iter().map(Value::key).collect::<Vec<_>>());
        let wanted = [
            Value::Bool(true),
            Value::Number(0.),
            Value::Number(f64::NAN),
            Value::text("a"),
            Value::text("b"),
            Value::text("absent"),
            Value::Null,
        ];
        let grids = [
            Grid::new(domain(&wanted), domain(&wanted[1..5])),
            Grid::new(domain(&wanted), None),
            Grid::new(None, domain(&wanted)),
        ];
        // The cell-by-cell scan the lookup replaced: a channel must match the
        // cell's key on its axis; an axis without a channel matches any cell.
        let fits = |channel: Option<&Channel>, have: &Option<ValueKey>, row: &Table| match channel {
            None => true,
            Some(channel) => have.as_ref() == Some(&channel.read(row).key()),
        };
        // The values the grid's build reads, which a partition then reuses.
        let mut reads = FacetReads::default();
        reads.read(&data, &fx);
        reads.read(&data, &fy);
        for grid in &grids {
            for (cx, cy) in [(Some(&fx), Some(&fy)), (Some(&fx), None), (None, Some(&fy))] {
                let groups = grid.partition_channels(&data, cx, cy);
                assert_eq!(groups, grid.partition_reading(&data, cx, cy, &reads));
                assert_eq!(groups.len(), grid.cells.len());
                for (cell, rows) in grid.cells.iter().zip(&groups) {
                    let expected: Vec<&Table> = data
                        .rows()
                        .iter()
                        .filter(|row| fits(cx, &cell.x, row) && fits(cy, &cell.y, row))
                        .collect();
                    assert_eq!(rows.iter().collect::<Vec<_>>(), expected, "{cell:?}");
                }
            }
        }
    }
}
