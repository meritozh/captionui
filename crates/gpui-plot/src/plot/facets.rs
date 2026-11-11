//! The facet grid: the facet scales' `interval` over the marks' facet
//! channels, and the `fx`/`fy` domains the grid reads from the data.

use std::rc::Rc;

use crate::facet::Grid;
use crate::mark::Mark;
use crate::plot::Plot;
use crate::table::{Dataset, Table};
use crate::value::Value;

impl Plot {
    /// The marks with Plot's facet-scale `interval` applied: each facet
    /// channel (the mark's own, or the plot's `fx`/`fy` field) reads its
    /// value's interval start, so `fx: {interval: 0.5}` facets by half-carat.
    pub(super) fn interval_marks(&self) -> Option<Vec<Mark>> {
        let (ix, iy) = (self.scales.fx.interval, self.scales.fy.interval);
        if ix.is_none() && iy.is_none() {
            return None;
        }
        // Marks that shared a facet channel share its wrapper, so the facet
        // partition (keyed on channel identity) is computed once for them.
        let wrapped: std::cell::RefCell<Vec<(bool, crate::table::Channel, crate::table::Channel)>> =
            Default::default();
        let wrap = |channel: crate::table::Channel, interval: crate::mark::Interval, x: bool| {
            if let Some((_, _, w)) = wrapped
                .borrow()
                .iter()
                .find(|(axis, original, _)| *axis == x && original.same(&channel))
            {
                return w.clone();
            }
            let original = channel.clone();
            // The facet channel is read over every record in turn, which
            // keep a field at one position.
            let hint = std::cell::Cell::new(0);
            let w = crate::table::Channel::Accessor(Rc::new(move |row: &Table| {
                let value = channel.read_hinted(row, &hint);
                match interval.extent(&value) {
                    Some((start, _)) => match value {
                        Value::Date(_) => Value::date_from_millis(start as i64),
                        _ => Value::Number((start * 1e9).round() / 1e9),
                    },
                    None => value,
                }
            }));
            wrapped.borrow_mut().push((x, original, w.clone()));
            w
        };
        Some(
            self.marks
                .iter()
                .map(|mark| {
                    let mut mark = mark.clone();
                    let plot_x = self
                        .fx
                        .clone()
                        .filter(|_| mark.data.is_none() && mark.fx.is_none() && mark.fy.is_none());
                    let plot_y = self
                        .fy
                        .clone()
                        .filter(|_| mark.data.is_none() && mark.fx.is_none() && mark.fy.is_none());
                    let fx = mark
                        .fx
                        .clone()
                        .or(plot_x.map(crate::table::Channel::Field));
                    let fy = mark
                        .fy
                        .clone()
                        .or(plot_y.map(crate::table::Channel::Field));
                    mark.fx = match (fx, ix) {
                        (Some(c), Some(i)) => Some(wrap(c, i, true)),
                        (c, _) => c,
                    };
                    mark.fy = match (fy, iy) {
                        (Some(c), Some(i)) => Some(wrap(c, i, false)),
                        (c, _) => c,
                    };
                    mark
                })
                .collect(),
        )
    }

    /// The facet grid the plot's `fx`/`fy` fields resolve.
    ///
    /// The domains come from the data's values, so a facet exists for every
    /// value a mark could draw there — the same values Plot's facet band
    /// scales read.
    pub(super) fn facet_grid(&self) -> Grid {
        let bucketed = self.interval_marks();
        self.facet_grid_with(bucketed.as_deref())
    }

    /// [`Plot::facet_grid`] given the plot's [`Plot::interval_marks`], so a
    /// caller that already has them doesn't compute them twice.
    fn facet_grid_with(&self, bucketed: Option<&[Mark]>) -> Grid {
        self.facet_grid_reading(bucketed, None)
    }

    /// [`Plot::facet_grid_with`], keeping the values each mark's facet
    /// channels read in `reads`, when given, for the facet partitions.
    pub(super) fn facet_grid_reading(
        &self,
        bucketed: Option<&[Mark]>,
        mut reads: Option<&mut crate::facet::FacetReads>,
    ) -> Grid {
        // The domains are every facet value a mark could draw: the plot's
        // `fx`/`fy` over its data, and each mark's own facet channels over
        // the mark's data. Only each key's first value is kept, since the
        // grid keeps distinct keys, and a (data, channel) pair already read
        // adds nothing.
        #[derive(Default)]
        struct Distinct {
            seen: rustc_hash::FxHashSet<crate::value::ValueKey>,
            values: Vec<Value>,
        }
        impl Distinct {
            fn push(&mut self, value: Value) {
                if self.seen.insert(value.key()) {
                    self.values.push(value);
                }
            }
            /// [`Distinct::push`] for a borrowed value, cloned only when it
            /// is new. A value equal to the last one kept has that one's key,
            /// already seen, so it is skipped without hashing.
            fn push_ref(&mut self, value: &Value) {
                if self.values.last() != Some(value) && self.seen.insert(value.key()) {
                    self.values.push(value.clone());
                }
            }
        }
        let mut fx = Distinct::default();
        let mut fy = Distinct::default();
        if bucketed.is_none() {
            if let Some(field) = &self.fx {
                self.data
                    .rows()
                    .iter()
                    .for_each(|row| fx.push(row.value(field)));
            }
            if let Some(field) = &self.fy {
                self.data
                    .rows()
                    .iter()
                    .for_each(|row| fy.push(row.value(field)));
            }
        }
        let mut read: Vec<(bool, &Dataset, &crate::table::Channel)> = Vec::new();
        for mark in bucketed.unwrap_or(&self.marks) {
            let data = mark.data.as_ref().unwrap_or(&self.data);
            for (x, channel, values) in [(true, &mark.fx, &mut fx), (false, &mark.fy, &mut fy)] {
                let Some(channel) = channel else { continue };
                if read
                    .iter()
                    .any(|(axis, d, c)| *axis == x && d.same_rows(data) && c.same(channel))
                {
                    continue;
                }
                read.push((x, data, channel));
                match reads.as_deref_mut() {
                    Some(reads) => reads
                        .read(data, channel)
                        .iter()
                        .for_each(|value| values.push_ref(value)),
                    None => data
                        .rows()
                        .iter()
                        .for_each(|row| values.push(channel.read(row))),
                }
            }
        }
        let (mut fx, mut fy) = (fx.values, fy.values);
        // Plot's interval domain is the range of the intervals between the
        // values' extent, so a missing value has no facet.
        if self.scales.fx.interval.is_some() {
            fx.retain(|v| !v.is_null());
        }
        if self.scales.fy.interval.is_some() {
            fy.retain(|v| !v.is_null());
        }
        // An explicit facet domain wins, in its order.
        let explicit =
            |options: &crate::options::ScaleOptions, values: Vec<Value>| match &options.domain {
                Some(domain) => domain.clone(),
                None => values,
            };
        let mut grid =
            Grid::from_values(explicit(&self.scales.fx, fx), explicit(&self.scales.fy, fy));
        if let (Some(domain), Some(keys)) = (&self.scales.fx.domain, grid.fx.as_mut()) {
            *keys = crate::scale::dedup(domain.iter().map(Value::key));
        }
        if let (Some(domain), Some(keys)) = (&self.scales.fy.domain, grid.fy.as_mut()) {
            *keys = crate::scale::dedup(domain.iter().map(Value::key));
        }
        // Plot's `reverse` on a facet scale reverses its domain.
        let mut rebuild = self.scales.fx.domain.is_some() || self.scales.fy.domain.is_some();
        for (options, keys) in [(&self.scales.fx, grid.fx.as_mut()), (&self.scales.fy, grid.fy.as_mut())] {
            if let (Some(true), Some(keys)) = (options.reverse, keys) {
                keys.reverse();
                rebuild = true;
            }
        }
        if rebuild {
            grid = Grid::new(grid.fx.clone(), grid.fy.clone());
        }
        grid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mark::MarkKind;
    use crate::render;

    #[test]
    fn facet_grid_domains_come_from_the_data() {
        let data = [
            Table::empty()
                .with("r", Value::text("a"))
                .with("c", Value::text("x")),
            Table::empty()
                .with("r", Value::text("b"))
                .with("c", Value::text("x")),
            Table::empty()
                .with("r", Value::text("a"))
                .with("c", Value::text("y")),
        ];
        let plot = Plot::new(data).dot(|mark| mark.x("c")).fx("r").fy("c");
        let grid = plot.facet_grid();
        // Both domains are distinct values in the order they appear, so the
        // grid is the cross product: two columns by two rows.
        assert_eq!(grid.fx_labels(), vec!["a", "b"]);
        assert_eq!(grid.fy_labels(), vec!["x", "y"]);
        assert_eq!(grid.columns(), 2);
        assert_eq!(grid.rows(), 2);
        assert_eq!(grid.cells.len(), 4);
    }

    #[test]
    fn facet_domains_read_every_distinct_source() {
        // Marks over different data, with the same facet field, both add
        // their values; one source read twice adds nothing.
        let source =
            |keys: &[&str]| Dataset::new(keys.iter().map(|k| Table::new([("k", Value::text(*k))])));
        let (a, b) = (source(&["c", "a"]), source(&["b", "a"]));
        let dot = |data: &Dataset| Mark::new(MarkKind::Dot).data(data.clone()).x("k");
        let plot = Plot::plot()
            .mark(dot(&a).fx("k"))
            .mark(dot(&b).fx("k"))
            .mark(dot(&a).fx("k"))
            .mark(dot(&b).fy("k"));
        let grid = plot.facet_grid();
        assert_eq!(grid.fx_labels(), vec!["a", "b", "c"]);
        assert_eq!(grid.fy_labels(), vec!["a", "b"]);
    }

    #[test]
    fn facet_interval_wrappers_are_shared_per_axis_and_channel() {
        let data: Vec<Table> = (0..30)
            .map(|i| {
                Table::new([
                    ("v", Value::Number(i as f64)),
                    ("w", Value::Number((i % 4) as f64)),
                ])
            })
            .collect();
        let plot = Plot::new(data.clone())
            .fx("v")
            .fx_scale(|s| s.interval(crate::mark::Interval::Step(10.)))
            .fy_scale(|s| s.interval(crate::mark::Interval::Step(4.)))
            .mark(Mark::new(MarkKind::Dot).x("w"))
            .mark(Mark::new(MarkKind::Dot).x("w").fx("v"))
            .mark(Mark::new(MarkKind::Dot).x("w").fx("v").fy("v"))
            .mark(Mark::new(MarkKind::Dot).x("w").fx("w"));
        let marks = plot.interval_marks().unwrap();
        let fx = |m: usize| marks[m].fx.clone().unwrap();
        // The plot's field and a mark's own field of that name share one
        // wrapper; another field, or the other axis, gets its own.
        assert!(fx(0).same(&fx(1)) && fx(1).same(&fx(2)));
        assert!(!fx(3).same(&fx(0)));
        let fy = marks[2].fy.clone().unwrap();
        assert!(!fy.same(&fx(2)));
        for row in &data {
            let v = row.num("v");
            assert_eq!(fx(2).read(row), Value::Number((v / 10.).floor() * 10.));
            assert_eq!(fy.read(row), Value::Number((v / 4.).floor() * 4.));
            assert_eq!(
                fx(3).read(row),
                Value::Number((row.num("w") / 10.).floor() * 10.)
            );
        }
        let grid = plot.facet_grid();
        assert_eq!(grid.fx_labels(), vec!["0", "10", "20"]);
        assert_eq!(grid.fy.as_ref().map(Vec::len), Some(8));
    }

    #[test]
    fn a_grid_that_keeps_its_reads_is_the_grid_and_partitions_alike() {
        // Facet keys in runs, where the grid skips a value equal to the last
        // one kept: -0 after +0, NaNs (an accessor keeps them), null, text
        // and two dates in one millisecond.
        let instant = |nanos: u32| {
            Value::Date(chrono::DateTime::from_timestamp(5, nanos).expect("a valid instant"))
        };
        let keys = [
            Value::Number(0.),
            Value::Number(-0.),
            Value::Number(f64::NAN),
            Value::Null,
            Value::text("a"),
            Value::text("a"),
            instant(100),
            instant(200_000),
            Value::Number(0.),
            Value::Bool(true),
        ];
        let data: Vec<Table> = (0..60)
            .map(|i| {
                Table::new([
                    ("k", keys[(i / 2) % keys.len()].clone()),
                    ("j", keys[(i / 5) % keys.len()].clone()),
                    ("v", Value::Number(i as f64)),
                ])
            })
            .collect();
        let accessor = |field: &'static str| {
            crate::table::Channel::Accessor(Rc::new(move |row: &Table| row.value(field)))
        };
        let (k, j) = (accessor("k"), accessor("j"));
        let plots = [
            Plot::new(data.clone())
                .mark(Mark::new(MarkKind::Dot).x("v").fx(k.clone()))
                .mark(Mark::new(MarkKind::Dot).x("v").fx(k.clone()).fy(j.clone())),
            Plot::new(data.clone())
                .mark(Mark::new(MarkKind::Dot).x("v").fx("k").fy("j"))
                .mark(Mark::new(MarkKind::Dot).x("v").fy("k")),
            Plot::new(data.clone())
                .fx("j")
                .fx_scale(|s| s.interval(crate::mark::Interval::Step(10.)))
                .mark(Mark::new(MarkKind::Dot).x("v").fx("v"))
                .mark(Mark::new(MarkKind::Dot).x("v")),
        ];
        for plot in plots {
            let bucketed = plot.interval_marks();
            let mut reads = crate::facet::FacetReads::default();
            let kept = plot.facet_grid_reading(bucketed.as_deref(), Some(&mut reads));
            let plain = plot.facet_grid_with(bucketed.as_deref());
            assert_eq!(kept.cells, plain.cells);
            assert_eq!(kept.fx, plain.fx);
            assert_eq!(kept.fy, plain.fy);
            let marks = bucketed.as_deref().unwrap_or(&plot.marks);
            let (with_reads, populated) = render::prepare_facets_reading(
                &plot.data,
                marks,
                &plain,
                plot.fx.as_deref(),
                None,
                &reads,
            );
            let (without, populated_without) =
                render::prepare_facets(&plot.data, marks, &plain, plot.fx.as_deref(), None);
            assert_eq!(populated, populated_without);
            assert_eq!(format!("{with_reads:?}"), format!("{without:?}"));
        }
    }
}
