//! One record read into a `PreparedRow`: in full, or only as far as the
//! positions and series grouping reads.

use crate::mark::{Axis, Channels, CrossAxis, Mark, MarkKind};
use crate::render::PreparedRow;
use crate::render::interval::derive_intervals;
use crate::table::Table;
use crate::value::Value;

/// Which positional channel a row's `z` shares, so the row reads it once.
#[derive(Clone, Copy)]
enum SharedZ {
    None,
    X,
    Y,
}

/// The facts [`resolve_row`] needs that depend only on the mark and its
/// channels, worked out once per mark rather than once per row.
pub(super) struct RowFacts {
    /// A one-dimensional mark fills its missing y (or x) with the row index.
    index_y: bool,
    index_x: bool,
    shared_z: SharedZ,
    /// A symbol channel that names a symbol is a constant.
    constant_symbol: Option<crate::style::Symbol>,
    r_unscaled: bool,
    opacity_unscaled: bool,
    /// Whether a raw row runs [`derive_intervals`]: see [`derives_intervals`].
    pub(super) derive: bool,
    /// Where the field each positional channel reads (x, y, x1, x2, y1, y2,
    /// z) sat in the last record, which the next record of the same shape
    /// shares: see [`Table::get_hinted`].
    slots: [std::cell::Cell<usize>; 7],
}

/// Whether [`derive_intervals`] on a mark's raw rows can change anything the
/// pipeline reads. Skipping it changes no result in two cases:
///
/// - It writes nothing: the mark has no `interval` and is none of the kinds
///   it grows an interval for.
/// - The mark groups, names no interval channel, and runs none of the
///   [`transforms_before_group`], which read or rewrite an interval. With no
///   interval channel a raw row's ends start empty, and every derivation
///   that fills one needs that axis's value to be non-null, so
///   [`PreparedRow::x_value`] and [`PreparedRow::y_value`] — all a sort, the
///   group keys and [`group_input`] read when no interval channel is named —
///   return the value either way. [`collapse_groups`] then drops the raw
///   rows' ends and derives each grouped row's afresh.
///
/// [`group_input`]: super::group::group_input
/// [`collapse_groups`]: super::group::collapse_groups
pub(super) fn derives_intervals(channels: &Channels, mark: &Mark) -> bool {
    use MarkKind::*;
    let writes = mark.interval.is_some()
        || matches!(
            mark.kind,
            RuleX
                | RuleY
                | DifferenceX
                | DifferenceY
                | BarX
                | BarY
                | AreaX
                | AreaY
                | WaffleX
                | WaffleY
                | RectX
                | RectY
                | Cell
        );
    let grouped_away = mark.group.is_some()
        && channels.x1.is_none()
        && channels.x2.is_none()
        && channels.y1.is_none()
        && channels.y2.is_none()
        && !transforms_before_group(mark);
    writes && !grouped_away
}

/// Whether any transform between resolving a mark's rows and grouping them,
/// besides its sort, applies: a window, a normalize (a `normalizeX`/`Y`
/// normalizes before only an unbinned mark), an outlier filter or a select
/// on an axis. Each reads or rewrites the rows' values and interval ends.
pub(super) fn transforms_before_group(mark: &Mark) -> bool {
    mark.window.is_some()
        || mark.normalize
        || (mark.normalize_by.is_some() && mark.bin.is_none())
        || mark.outliers.is_some()
        || mark
            .select
            .as_ref()
            .is_some_and(|config| config.axis.is_some())
}

impl RowFacts {
    pub(super) fn new(channels: &Channels, mark: &Mark) -> Self {
        // A one-dimensional mark has no second position channel. Plot fills it
        // with the datum's index for `lineX`/`rectX`, and leaves it unset for
        // `dotX`, whose dots all sit in the frame's middle.
        // A cross channel the mark names wins: `Plot.lineY(data, {x, y})`.
        let category = mark.kind.category();
        let index_cross = category.cross == CrossAxis::Index;
        let index_y = category.single_axis == Some(Axis::X) && index_cross && channels.y.is_none();
        let index_x = !index_y
            && category.single_axis == Some(Axis::Y)
            && index_cross
            && channels.x.is_none();
        // A box part's `z` is its category channel, which is also its x (or
        // y): reuse that read rather than calling the channel twice. Constants
        // are cheap to read, so only fields and accessors are shared.
        let shared_z = match &channels.z {
            Some(crate::table::Channel::Constant(_)) | None => SharedZ::None,
            Some(z) if channels.x.as_ref().is_some_and(|x| z.same(x)) => SharedZ::X,
            Some(z) if channels.y.as_ref().is_some_and(|y| z.same(y)) => SharedZ::Y,
            Some(_) => SharedZ::None,
        };
        Self {
            index_y,
            index_x,
            shared_z,
            constant_symbol: channels.symbol.as_ref().and_then(constant_symbol),
            r_unscaled: channels.r.as_ref().is_some_and(|c| !c.is_scaled()),
            opacity_unscaled: channels.opacity.as_ref().is_some_and(|c| !c.is_scaled()),
            derive: derives_intervals(channels, mark),
            slots: Default::default(),
        }
    }
}

// Inlined into each loop that resolves rows, which then builds every row
// where it lands rather than copying it out of a call.
#[inline(always)]
pub(super) fn resolve_row(
    row: &Table,
    index: usize,
    channels: &Channels,
    mark: &Mark,
    facts: &RowFacts,
) -> PreparedRow {
    let core = resolve_core(row, index, channels, mark, facts);
    finish_row(core, row, index, channels, facts)
}

/// What grouping and the outlier filter read of a row besides its record:
/// its positions, interval ends and series, as [`resolve_row`] resolves
/// them (it resolves these first, then [`finish_row`] the rest).
pub(super) struct RowCore {
    pub(super) x: Value,
    pub(super) y: Value,
    x1: Option<Value>,
    x2: Option<Value>,
    y1: Option<Value>,
    y2: Option<Value>,
    pub(super) z: Option<Value>,
}

/// A row read as far as its [`RowCore`], with its record and index, which
/// [`finish_row`] makes a [`PreparedRow`] only if it is kept.
pub(super) struct CoreRow<'a> {
    pub(super) core: RowCore,
    pub(super) record: &'a Table,
    pub(super) index: usize,
}

/// A row as grouping reads it: a [`PreparedRow`], or a [`CoreRow`] not yet
/// resolved in full.
pub(super) trait GroupRow {
    fn record(&self) -> &Table;
    fn x(&self) -> &Value;
    fn y(&self) -> &Value;
    fn x1(&self) -> Option<&Value>;
    fn x2(&self) -> Option<&Value>;
    fn y1(&self) -> Option<&Value>;
    fn y2(&self) -> Option<&Value>;
    fn z(&self) -> Option<&Value>;

    /// [`PreparedRow::x_value`].
    fn x_value(&self) -> &Value {
        let x = self.x();
        if !x.is_null() {
            x
        } else {
            self.x1().unwrap_or(x)
        }
    }

    /// [`PreparedRow::y_value`].
    fn y_value(&self) -> &Value {
        let y = self.y();
        if !y.is_null() {
            y
        } else {
            self.y1().unwrap_or(y)
        }
    }
}

impl GroupRow for PreparedRow {
    fn record(&self) -> &Table {
        &self.row
    }
    fn x(&self) -> &Value {
        &self.x
    }
    fn y(&self) -> &Value {
        &self.y
    }
    fn x1(&self) -> Option<&Value> {
        self.x1.as_ref()
    }
    fn x2(&self) -> Option<&Value> {
        self.x2.as_ref()
    }
    fn y1(&self) -> Option<&Value> {
        self.y1.as_ref()
    }
    fn y2(&self) -> Option<&Value> {
        self.y2.as_ref()
    }
    fn z(&self) -> Option<&Value> {
        self.z.as_ref()
    }
}

impl GroupRow for CoreRow<'_> {
    fn record(&self) -> &Table {
        self.record
    }
    fn x(&self) -> &Value {
        &self.core.x
    }
    fn y(&self) -> &Value {
        &self.core.y
    }
    fn x1(&self) -> Option<&Value> {
        self.core.x1.as_ref()
    }
    fn x2(&self) -> Option<&Value> {
        self.core.x2.as_ref()
    }
    fn y1(&self) -> Option<&Value> {
        self.core.y1.as_ref()
    }
    fn y2(&self) -> Option<&Value> {
        self.core.y2.as_ref()
    }
    fn z(&self) -> Option<&Value> {
        self.core.z.as_ref()
    }
}

impl<T: GroupRow> GroupRow for &T {
    fn record(&self) -> &Table {
        (**self).record()
    }
    fn x(&self) -> &Value {
        (**self).x()
    }
    fn y(&self) -> &Value {
        (**self).y()
    }
    fn x1(&self) -> Option<&Value> {
        (**self).x1()
    }
    fn x2(&self) -> Option<&Value> {
        (**self).x2()
    }
    fn y1(&self) -> Option<&Value> {
        (**self).y1()
    }
    fn y2(&self) -> Option<&Value> {
        (**self).y2()
    }
    fn z(&self) -> Option<&Value> {
        (**self).z()
    }
}

/// The first half of [`resolve_row`]: the channels a [`RowCore`] holds,
/// read in the order `resolve_row` reads them.
#[inline(always)]
pub(super) fn resolve_core(
    row: &Table,
    index: usize,
    channels: &Channels,
    mark: &Mark,
    facts: &RowFacts,
) -> RowCore {
    let read = |channel: &Option<crate::table::Channel>, slot: usize| match channel {
        Some(channel @ crate::table::Channel::Field(_)) => {
            channel.read_hinted(row, &facts.slots[slot])
        }
        Some(channel) => channel.read(row),
        None => Value::Null,
    };

    let mut x = read(&channels.x, 0);
    let mut y = read(&channels.y, 1);
    let shared_z = match facts.shared_z {
        SharedZ::None => None,
        SharedZ::X => Some(x.clone()),
        SharedZ::Y => Some(y.clone()),
    };
    let x1 = read(&channels.x1, 2);
    let x2 = read(&channels.x2, 3);
    let y1 = read(&channels.y1, 4);
    let y2 = read(&channels.y2, 5);

    let mut x1 = (!x1.is_null()).then_some(x1);
    let mut x2 = (!x2.is_null()).then_some(x2);
    let mut y1 = (!y1.is_null()).then_some(y1);
    let mut y2 = (!y2.is_null()).then_some(y2);
    if facts.derive {
        derive_intervals(mark, &x, &y, &mut x1, &mut x2, &mut y1, &mut y2);
    }

    if facts.index_y {
        y = Value::Number(index as f64);
    } else if facts.index_x {
        x = Value::Number(index as f64);
    }

    let z = match shared_z {
        Some(value) => Some(value),
        None => channels.z.as_ref().map(|_| read(&channels.z, 6)),
    }
    .filter(|v| !v.is_null());
    RowCore {
        x,
        y,
        x1,
        x2,
        y1,
        y2,
        z,
    }
}

/// The second half of [`resolve_row`]: the row from its core and the
/// channels left to read.
#[inline(always)]
pub(super) fn finish_row(
    core: RowCore,
    row: &Table,
    index: usize,
    channels: &Channels,
    facts: &RowFacts,
) -> PreparedRow {
    let RowCore {
        x,
        y,
        x1,
        x2,
        y1,
        y2,
        z,
    } = core;
    PreparedRow {
        row: row.clone(),
        index,
        x,
        y,
        x1,
        x2,
        y1,
        y2,
        z,
        path: channels
            .path
            .as_ref()
            .map(|channel| channel.read(row))
            .filter(|v| !v.is_null())
            .map(|v| v.label()),
        r: channels
            .r
            .as_ref()
            .filter(|_| facts.r_unscaled)
            .map(|channel| channel.read(row)),
        r_value: channels
            .r
            .as_ref()
            .and_then(|channel| channel.data_value(row)),
        opacity_value: channels
            .opacity
            .as_ref()
            .and_then(|channel| channel.data_value(row)),
        symbol: channels.symbol.as_ref().and_then(|channel| {
            facts
                .constant_symbol
                .or_else(|| symbol_from_value(&channel.read(row)))
        }),
        opacity: channels
            .opacity
            .as_ref()
            .filter(|_| facts.opacity_unscaled)
            .map(|channel| channel.read(row)),
        dodge_offset: None,
        pixel: None,
    }
}

/// A symbol channel's value as a [`Symbol`](crate::style::Symbol): a name
/// (`"circle"`, `"square"`, …) or an index into the symbol set.
/// A symbol channel that names a symbol (`symbol: "square"`) is a constant,
/// as Plot reads it.
pub fn constant_symbol(channel: &crate::table::Channel) -> Option<crate::style::Symbol> {
    match channel {
        crate::table::Channel::Field(name) => crate::style::Symbol::parse(name),
        crate::table::Channel::Constant(value) => symbol_from_value(value),
        _ => None,
    }
}

pub(super) fn symbol_from_value(value: &Value) -> Option<crate::style::Symbol> {
    match value {
        Value::Text(text) => crate::style::Symbol::parse(text),
        _ => None,
    }
}
