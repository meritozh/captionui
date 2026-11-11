//! What the position scales read from the marks: their x and y values,
//! a `sort`-ordered domain, whether an axis is read at all, and whether
//! it takes bands.

use crate::mark::MarkKind;
use crate::plot::Plot;
use crate::render::{self, PreparedMark};
use crate::table::{Dataset, Table};
use crate::value::Value;

impl Plot {
    /// Whether any data mark reads a position on the axis, which is when Plot
    /// draws it: a plot of `dotX` has no y axis. A radial mark lays its rows
    /// out on its own and reads neither.
    pub(super) fn reads_axis(&self, x: bool) -> bool {
        // A scale given a type or domain exists, and Plot draws its axis.
        let options = if x { &self.scales.x } else { &self.scales.y };
        if options.r#type.is_some() || options.domain.is_some() {
            return true;
        }
        let grid_kind = if x { MarkKind::GridX } else { MarkKind::GridY };
        if self.marks.iter().any(|mark| mark.kind == grid_kind) {
            return true;
        }
        self.marks.iter().any(|mark| {
            if mark.kind.is_furniture()
                || matches!(mark.kind, MarkKind::Pie | MarkKind::Arc | MarkKind::Waffle)
            {
                return false;
            }
            // The channels Plot's value-data defaults give the mark count.
            let defaulted = mark
                .data
                .as_ref()
                .and_then(|data| render::with_value_defaults(mark, data));
            let mark = defaulted.as_ref().unwrap_or(mark);
            let c = &mark.channels;
            // An explicit null channel, a box's missing category, reads
            // nothing.
            let set = |channel: &Option<crate::table::Channel>| {
                channel.as_ref().is_some_and(|channel| {
                    !matches!(channel, crate::table::Channel::Constant(Value::Null))
                })
            };
            let named = if x {
                set(&c.x) || c.x1.is_some() || c.x2.is_some()
            } else {
                set(&c.y) || c.y1.is_some() || c.y2.is_some()
            };
            let explicit_null = if x {
                c.x.is_some() && !set(&c.x) && c.x1.is_none() && c.x2.is_none()
            } else {
                c.y.is_some() && !set(&c.y) && c.y1.is_none() && c.y2.is_none()
            };
            if explicit_null {
                return false;
            }
            // A group or bin output to the axis (`groupZ({y: "count"})`)
            // gives the mark that channel too; a `groupX` with no outputs
            // counts into y.
            let letter = if x { "x" } else { "y" };
            let output_named = mark
                .group
                .iter()
                .flat_map(|g| g.outputs.iter())
                .chain(mark.bin.iter().flat_map(|b| b.outputs.iter()))
                .any(|o| {
                    o.name
                        .strip_prefix(letter)
                        .is_some_and(|r| matches!(r, "" | "1" | "2"))
                })
                || mark.group.as_ref().is_some_and(|g| {
                    g.outputs.is_empty()
                        && g.axis
                            == Some(if x {
                                crate::mark::Axis::Y
                            } else {
                                crate::mark::Axis::X
                            })
                });
            let named = named || output_named;
            // A mark over plain values reads its single axis by default, and a
            // one-dimensional mark spreads along the other by index.
            let single = mark.kind.category().single_axis;
            let by_value = mark
                .data
                .as_ref()
                .is_some_and(|data| data.is_values() || data.is_tuples());
            let indexed = mark.kind.cross_axis() == crate::mark::CrossAxis::Index
                && single
                    == Some(if x {
                        crate::mark::Axis::Y
                    } else {
                        crate::mark::Axis::X
                    });
            let cross_rule = matches!(
                (mark.kind, x),
                (MarkKind::RuleX | MarkKind::TickX | MarkKind::BarX, true)
                    | (MarkKind::RuleY | MarkKind::TickY | MarkKind::BarY, false)
            );
            // A raster or contour over a grid reads both axes.
            let gridded = matches!(mark.kind, MarkKind::Raster | MarkKind::Contour)
                && crate::render::raster_extent(mark).is_some();
            // A group writes its outputs to the axes they name, and its
            // default count to the axis the grouping leaves free.
            let grouped = mark.group.as_ref().is_some_and(|group| {
                let names: &[&str] = if x {
                    &["x", "x1", "x2"]
                } else {
                    &["y", "y1", "y2"]
                };
                let by_x = group.both || group.axis == Some(crate::mark::Axis::X);
                let by_y = group.both || group.axis == Some(crate::mark::Axis::Y);
                (group.outputs.is_empty() && ((!x && by_x && !by_y) || (x && by_y && !by_x)))
                    || group.outputs.iter().any(|o| names.contains(&o.name.as_ref()))
            });
            named
                || grouped
                || gridded
                || indexed
                || (by_value
                    && (single
                        == Some(if x {
                            crate::mark::Axis::X
                        } else {
                            crate::mark::Axis::Y
                        })
                        || cross_rule
                        || mark.data.as_ref().is_some_and(Dataset::is_tuples)))
                || mark.geo_geometry.is_some()
        })
    }

    pub(super) fn x_domain_values(&self, facets: &[Vec<PreparedMark>]) -> Vec<Value> {
        facets
            .iter()
            .flatten()
            .flat_map(render::basic::x_values)
            .collect()
    }

    pub(super) fn y_domain_values(&self, facets: &[Vec<PreparedMark>]) -> Vec<Value> {
        facets
            .iter()
            .flatten()
            .flat_map(render::basic::y_values)
            .collect()
    }

    /// [`Plot::y_domain_values`], lazily.
    pub(super) fn y_domain_iter<'a>(
        &self,
        facets: &'a [Vec<PreparedMark>],
    ) -> impl Iterator<Item = Value> + 'a {
        facets.iter().flatten().flat_map(render::basic::y_domain)
    }
}

/// The domain a mark's `sort: {scale: by}` option orders, Plot's
/// `sort` channel option: each of the scale's values is ranked by the `by`
/// channel's values on its rows, reduced (by `max` unless said otherwise).
pub(crate) fn sorted_domain(facets: &[Vec<PreparedMark>], scale: &str) -> Option<Vec<Value>> {
    let mut ranked: Vec<(Value, Vec<Value>)> = Vec::new();
    let mut sort: Option<crate::mark::DomainSort> = None;
    for prepared in facets.iter().flatten() {
        let Some(spec) = prepared.mark.domain_sort.iter().find(|s| s.scale == scale) else {
            continue;
        };
        sort.get_or_insert_with(|| spec.clone());
        for (index, row) in prepared.rows.iter().enumerate() {
            let key = match scale {
                "x" => row.x_value().clone(),
                "y" => row.y_value().clone(),
                "color" => match prepared.mark.channels.fill.as_ref().or(prepared
                    .mark
                    .channels
                    .stroke
                    .as_ref())
                {
                    Some(channel) => channel
                        .field()
                        .map(|f| row.row.value(f))
                        .unwrap_or(Value::Null),
                    None => Value::Null,
                },
                _ => Value::Null,
            };
            if key.is_null() {
                continue;
            }
            let by = match spec.by.as_ref() {
                "x" => row.x_value().clone(),
                "x1" => row.x1.clone().unwrap_or_else(|| row.x_value().clone()),
                "x2" => row.x2.clone().unwrap_or_else(|| row.x_value().clone()),
                "y" => row.y_value().clone(),
                "y1" => row.y1.clone().unwrap_or_else(|| row.y_value().clone()),
                "y2" => row.y2.clone().unwrap_or_else(|| row.y_value().clone()),
                "r" => row.r_value.map(Value::Number).unwrap_or(Value::Null),
                // A bar's extent along its value axis, Plot's `width` and
                // `height` sort channels.
                "width" | "height" => {
                    let (a, b) = if spec.by.as_ref() == "width" {
                        (&row.x1, &row.x2)
                    } else {
                        (&row.y1, &row.y2)
                    };
                    match (
                        a.as_ref().and_then(Value::as_number),
                        b.as_ref().and_then(Value::as_number),
                    ) {
                        (Some(a), Some(b)) => Value::Number((b - a).abs()),
                        _ => Value::Null,
                    }
                }
                "data" => Value::Number(index as f64),
                // The color channels' data values, before their scale.
                "fill" | "stroke" => {
                    let channels = &prepared.mark.channels;
                    let channel = if spec.by.as_ref() == "fill" {
                        &channels.fill
                    } else {
                        &channels.stroke
                    };
                    channel
                        .as_ref()
                        .and_then(|channel| channel.scaled_value(&row.row))
                        .unwrap_or(Value::Null)
                }
                "z" => row.z.clone().unwrap_or(Value::Null),
                // Plot sorts `x` by `-y` as `{x: "y", reverse: true}`; any
                // other name reads the record's field.
                field => row.row.value(field),
            };
            match ranked.iter_mut().find(|(k, _)| k.key() == key.key()) {
                Some((_, values)) => values.push(by),
                None => ranked.push((key, vec![by])),
            }
        }
    }
    let sort = sort?;
    let mut scored: Vec<(Value, Value)> = ranked
        .into_iter()
        .map(|(key, values)| {
            let records: Vec<&Table> = Vec::new();
            let reduced = sort.reduce.reduce(&values, &records);
            (key, reduced)
        })
        .collect();
    // Plot's `ascendingDefined`: missing values last, whichever the order.
    // Plot's `ascendingGroup`/`descendingGroup`: ties fall back to the
    // keys, ascending.
    scored.sort_by(|(ka, a), (kb, b)| {
        match (a.is_null(), b.is_null()) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => {
                let ordering = a.cmp(b);
                if sort.reverse {
                    ordering.reverse()
                } else {
                    ordering
                }
            }
        }
        .then_with(|| ka.cmp(kb))
    });
    let mut domain: Vec<Value> = scored.into_iter().map(|(key, _)| key).collect();
    if let Some(limit) = sort.limit {
        domain.truncate(limit);
    }
    Some(domain)
}

/// Whether any mark draws a band along the x axis (`along_x`) or the y axis.
///
/// Only matters for a discrete axis: a continuous scale has no bands.
pub(super) fn marks_use_band(facets: &[Vec<PreparedMark>], along_x: bool) -> bool {
    facets.iter().flatten().any(|prepared| {
        let discrete = if along_x {
            prepared
                .rows
                .iter()
                .find(|row| !row.x_value().is_null())
                .is_some_and(|row| !row.x_value().kind().is_quantitative())
        } else {
            prepared
                .rows
                .iter()
                .find(|row| !row.y_value().is_null())
                .is_some_and(|row| !row.y_value().kind().is_quantitative())
        };
        // Plot's bar and cell marks read their band axis as ordinal whatever
        // its values, so a bar per year takes a band per year.
        let forced = match prepared.mark.kind {
            MarkKind::BarY | MarkKind::Cell | MarkKind::CellX if along_x => {
                prepared.mark.channels.x.is_some() && prepared.mark.interval.is_none()
            }
            MarkKind::BarX | MarkKind::Cell | MarkKind::CellY if !along_x => {
                prepared.mark.channels.y.is_some() && prepared.mark.interval.is_none()
            }
            _ => false,
        };
        if forced {
            return true;
        }
        if !discrete {
            return false;
        }
        // A mark's band direction is part of its classification: a bar's
        // width, a cell's extent, a tick's ordinal secondary axis and a
        // box/violin/density's category axis all fill a band.
        let cat = prepared.mark.kind.category();
        if along_x { cat.band_x } else { cat.band_y }
    })
}
