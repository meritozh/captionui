//! A deterministic text dump of what the pipeline resolves, for the identity
//! harness `demo/examples/dump.rs`: two builds whose dumps match resolved the
//! same rows in the same order, with the same records, channels, scales and
//! colors.
//!
//! Numbers print through `{:?}`, which round-trips and tells `-0.0` and `NaN`
//! apart. Nothing prints an address: a geometry prints a digest of its
//! coordinates and a geometry key prints as null (the color scales print
//! through `Debug`, whose keys a color channel never makes geometries).

use std::fmt::Write;

use gpui::{Bounds, Hsla, Pixels};

use crate::facet::Grid;
use crate::frame::{Axis, Frame, Insets};
use crate::geo::{Fitted, Geometry};
use crate::render::{PreparedMark, PreparedRow};
use crate::scale::PositionScale;
use crate::table::Table;
use crate::value::{Value, ValueKey};

/// Fields the first optimization round removed from the records: skipped so
/// a dump from before it compares with one from after.
fn internal(name: &str) -> bool {
    name.starts_with("$in:") || name.starts_with("$box:")
}

/// FNV-1a over bytes: a stable digest, unlike `std`'s randomly keyed hasher.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ *byte as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

pub(crate) fn geometry(out: &mut String, geometry: &Geometry) {
    // The derived `Debug` prints every coordinate exactly and the nesting.
    let text = format!("{geometry:?}");
    let kind = text.split(['(', ' ']).next().unwrap_or("");
    let _ = write!(
        out,
        "geo({kind} len={} h={:016x})",
        text.len(),
        fnv(text.as_bytes())
    );
}

pub(crate) fn value(out: &mut String, value: &Value) {
    let _ = match value {
        Value::Null => write!(out, "null"),
        Value::Number(n) => write!(out, "{n:?}"),
        Value::Date(d) => write!(out, "date({d:?})"),
        Value::Text(t) => write!(out, "{:?}", t.as_ref()),
        Value::Bool(b) => write!(out, "{b}"),
        Value::Geo(g) => {
            geometry(out, g);
            Ok(())
        }
    };
}

pub(crate) fn key(out: &mut String, key: &ValueKey) {
    value(out, &key.to_value());
}

pub(crate) fn values<'a>(out: &mut String, values: impl IntoIterator<Item = &'a Value>) {
    out.push('[');
    for (i, v) in values.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        value(out, v);
    }
    out.push(']');
}

/// A long list of values under a heading, twenty to a line with the first
/// one's offset, so a diff points at where two lists part.
pub(crate) fn value_lines(out: &mut String, indent: &str, heading: &str, list: &[Value]) {
    let _ = writeln!(out, "{indent}{heading} n={}", list.len());
    for (chunk, values) in list.chunks(20).enumerate() {
        let _ = write!(out, "{indent}  @{} ", chunk * 20);
        self::values(out, values);
        out.push('\n');
    }
}

pub(crate) fn keys<'a>(out: &mut String, keys: impl IntoIterator<Item = &'a ValueKey>) {
    out.push('[');
    for (i, k) in keys.into_iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        key(out, k);
    }
    out.push(']');
}

fn optional<T>(out: &mut String, value: Option<T>, mut write: impl FnMut(&mut String, T)) {
    match value {
        Some(value) => write(out, value),
        None => out.push('-'),
    }
}

pub(crate) fn hsla(out: &mut String, color: Hsla) {
    let _ = write!(
        out,
        "hsla({:?},{:?},{:?},{:?})",
        color.h, color.s, color.l, color.a
    );
}

pub(crate) fn bounds(out: &mut String, bounds: Bounds<Pixels>) {
    let _ = write!(
        out,
        "[{:?}, {:?}, {:?}, {:?}]",
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32()
    );
}

fn insets(out: &mut String, insets: &Insets) {
    let _ = write!(
        out,
        "[{:?}, {:?}, {:?}, {:?}]",
        insets.top, insets.right, insets.bottom, insets.left
    );
}

/// The grid's facet domains and cells, with which cells the marks populated
/// (`natural_height`'s view) or which the grid leaves empty (`prepaint`'s).
pub(crate) fn grid(out: &mut String, grid: &Grid, populated: Option<&[bool]>) {
    out.push_str("grid fx=");
    optional(out, grid.fx.as_ref(), keys);
    out.push_str(" fy=");
    optional(out, grid.fy.as_ref(), keys);
    let _ = writeln!(out, " faceted={}", grid.is_faceted());
    for (i, cell) in grid.cells.iter().enumerate() {
        let _ = write!(out, "  cell {i} x=");
        optional(out, cell.x.as_ref(), key);
        out.push_str(" y=");
        optional(out, cell.y.as_ref(), key);
        match populated {
            Some(populated) => {
                let _ = write!(out, " populated={:?}", populated.get(i));
            }
            None => {
                let _ = write!(out, " empty={}", grid.is_empty(i));
            }
        }
        out.push('\n');
    }
    if let Some(populated) = populated
        && populated.len() != grid.cells.len()
    {
        let _ = writeln!(out, "  populated={populated:?}");
    }
}

/// A record's fields, in their (sorted) order.
pub(crate) fn record(out: &mut String, record: &Table) {
    for name in record.field_names() {
        if internal(name) {
            continue;
        }
        let _ = write!(out, " {name}=");
        optional(out, record.get(name), value);
    }
}

/// One row: its channels, then its record.
pub(crate) fn row(out: &mut String, i: usize, row: &PreparedRow) {
    let _ = write!(out, "    row {i} index={} x=", row.index);
    value(out, &row.x);
    out.push_str(" y=");
    value(out, &row.y);
    for (name, v) in [
        ("x1", &row.x1),
        ("x2", &row.x2),
        ("y1", &row.y1),
        ("y2", &row.y2),
        ("z", &row.z),
    ] {
        let _ = write!(out, " {name}=");
        optional(out, v.as_ref(), value);
    }
    let _ = write!(
        out,
        " path={:?} r={:?} symbol={:?} opacity={:?} r_value={:?} opacity_value={:?} \
         dodge_offset={:?} pixel={:?} |",
        row.path.as_ref().map(|p| p.as_ref()),
        row.r,
        row.symbol,
        row.opacity,
        row.r_value,
        row.opacity_value,
        row.dodge_offset,
        row.pixel,
    );
    record(out, &row.row);
    out.push('\n');
}

/// A mark's own resolved state, before its rows.
pub(crate) fn mark(out: &mut String, i: usize, mark: &PreparedMark) {
    let _ = writeln!(
        out,
        "  mark {i} {} rows={} dodge_along_x={} density_max={:?}",
        mark.mark.kind.name(),
        mark.rows.len(),
        mark.dodge_along_x,
        mark.density_max,
    );
    let [fx, fy] = &mark.filtered_domain;
    if !fx.is_empty() || !fy.is_empty() {
        value_lines(out, "    ", "filtered_domain x", fx);
        value_lines(out, "    ", "filtered_domain y", fy);
    }
    if let Some(raster) = &mark.raster {
        let heading = format!(
            "raster {}x{} x1={:?} y1={:?} x2={:?} y2={:?} values",
            raster.width, raster.height, raster.x1, raster.y1, raster.x2, raster.y2
        );
        value_lines(out, "    ", &heading, &raster.values);
    }
}

pub(crate) fn position_scale(out: &mut String, scale: &PositionScale) {
    match scale {
        PositionScale::Continuous(scale) => {
            let _ = write!(out, "{scale:?}");
        }
        PositionScale::Band(scale) => {
            out.push_str("band domain=");
            keys(out, scale.domain());
            let _ = write!(
                out,
                " range={:?} band_width={:?} step={:?}",
                scale.range(),
                scale.band_width(),
                scale.step()
            );
        }
        PositionScale::Point(scale) => {
            out.push_str("point domain=");
            keys(out, scale.domain());
            let _ = write!(out, " range={:?} step={:?}", scale.range(), scale.step());
        }
    }
}

fn axis(out: &mut String, name: &str, axis: &Axis) {
    let _ = write!(
        out,
        "axis {name} kind={:?} temporal={} categories={} scale=",
        axis.kind, axis.temporal, axis.categories
    );
    position_scale(out, &axis.scale);
    out.push('\n');
    for tick in &axis.ticks {
        out.push_str("  tick ");
        value(out, &tick.value);
        let _ = writeln!(out, " at {:?} {:?}", tick.position, tick.label.as_ref());
    }
}

fn projection(out: &mut String, fitted: &Fitted) {
    let p = &fitted.projection;
    let _ = write!(
        out,
        "projection kind={:?} rotate={:?} inset={:?} parallels={:?} center={:?} \
         clip_angle={:?} scale={:?} translate={:?} domain=",
        p.kind,
        p.rotate,
        p.inset,
        p.parallels,
        p.center,
        p.clip_angle,
        fitted.scale,
        fitted.translate
    );
    optional(out, p.domain.as_deref(), geometry);
    out.push('\n');
}

/// The frame's geometry, scales and ticks.
pub(crate) fn frame(out: &mut String, frame: &Frame) {
    out.push_str("frame bounds=");
    bounds(out, frame.bounds);
    out.push_str(" plot=");
    bounds(out, frame.plot);
    out.push_str(" insets=");
    insets(out, &frame.insets);
    out.push_str(" facet_insets=");
    insets(out, &frame.facet_insets);
    let _ = writeln!(out, " layout={:?}", frame.layout);
    axis(out, "x", &frame.x);
    axis(out, "y", &frame.y);
    if let Some(fitted) = &frame.projection {
        projection(out, fitted);
    }
}
