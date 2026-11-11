//! The typed mark builders (`DotMark`, `LineMark`, …): one generic
//! [`TypedMark`] over a zero-sized kind marker, aliased once per mark type in
//! one file per renderer family.

mod basic;
mod delaunay;
mod furniture;
mod radial;

use std::marker::PhantomData;

use crate::mark::{Mark, MarkKind, MarkOptions};

pub use basic::*;
pub use delaunay::*;
pub use furniture::*;
pub use radial::*;

/// The zero-sized kind markers that parameterize [`TypedMark`], one per typed
/// builder and each named after its [`MarkKind`]: [`DotMark`] is
/// `TypedMark<kinds::Dot>`.
pub mod kinds {
    pub use super::basic::kinds::*;
    pub use super::delaunay::kinds::*;
    pub use super::furniture::kinds::*;
    pub use super::radial::kinds::*;
}

/// A zero-sized type standing for one [`MarkKind`], the parameter of
/// [`TypedMark`]; the markers live in [`kinds`].
pub trait KindMarker {
    /// The kind of the marks the builder makes.
    const KIND: MarkKind;
    /// The builder's name, `DotMark`, which its `Debug` output carries.
    const NAME: &'static str;
}

/// A typed mark builder: a [`Mark`] whose kind the marker `K` fixes, so
/// `DotMark::new()` makes a dot. Its options come from the builder traits over
/// [`MarkOptions`], and reads go through `Deref` to the mark.
///
/// Each mark type has an alias, [`DotMark`] for `TypedMark<kinds::Dot>` and so
/// on.
pub struct TypedMark<K>(pub Mark, PhantomData<K>);

impl<K: KindMarker> TypedMark<K> {
    /// A mark of this kind with default channels and style.
    pub fn new() -> Self {
        Self(Mark::new(K::KIND), PhantomData)
    }
}

impl<K> TypedMark<K> {
    /// Wrap an already-configured [`Mark`].
    pub fn from_mark(mark: Mark) -> Self {
        Self(mark, PhantomData)
    }

    /// The mark this builder configures.
    pub fn into_mark(self) -> Mark {
        self.0
    }
}

impl<K: KindMarker> Default for TypedMark<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> Clone for TypedMark<K> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

impl<K: KindMarker> std::fmt::Debug for TypedMark<K> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(K::NAME).field(&self.0).finish()
    }
}

// Reads go through `Deref` so a builder's channels and style can be inspected;
// the builder methods take and return the builder, so a chain keeps its type.
impl<K> std::ops::Deref for TypedMark<K> {
    type Target = Mark;

    fn deref(&self) -> &Mark {
        &self.0
    }
}

impl<K> From<TypedMark<K>> for Mark {
    fn from(builder: TypedMark<K>) -> Mark {
        builder.0
    }
}

impl<K> From<Mark> for TypedMark<K> {
    fn from(mark: Mark) -> Self {
        Self(mark, PhantomData)
    }
}

impl<K> MarkOptions for TypedMark<K> {
    fn mark_mut(&mut self) -> &mut Mark {
        &mut self.0
    }
}

/// Declares a renderer family's typed builders: each one's kind marker, named
/// after its [`MarkKind`] variant, in the family's `kinds` module, and its
/// [`TypedMark`] alias.
macro_rules! typed_marks {
    ($($(#[$meta:meta])* $alias:ident = $kind:ident;)*) => {
        /// The kind markers of this family's typed builders.
        pub mod kinds {
            $(
                #[doc = concat!(
                    "The kind marker of [`", stringify!($alias), "`](crate::mark::",
                    stringify!($alias), "): [`MarkKind::", stringify!($kind),
                    "`](crate::mark::MarkKind::", stringify!($kind), ")."
                )]
                #[derive(Clone, Copy, Debug, Default)]
                pub struct $kind;

                impl crate::mark::KindMarker for $kind {
                    const KIND: crate::mark::MarkKind = crate::mark::MarkKind::$kind;
                    const NAME: &'static str = stringify!($alias);
                }
            )*
        }

        $(
            $(#[$meta])*
            pub type $alias = crate::mark::TypedMark<kinds::$kind>;
        )*
    };
}

use typed_marks;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::*;

    #[test]
    fn new_makes_a_mark_of_the_alias_kind() {
        assert_eq!(DotMark::new().kind, MarkKind::Dot);
        assert_eq!(VoronoiMeshMark::new().kind, MarkKind::VoronoiMesh);
        assert_eq!(BollingerYMark::default().kind, MarkKind::BollingerY);
        // `new` is `Mark::new`, so a kind keeps its own default style.
        assert_eq!(HexgridMark::new().style.stroke_opacity, 0.1);
        assert_eq!(
            format!("{:?}", AxisXMark::new()),
            format!("AxisXMark({:?})", Mark::new(MarkKind::AxisX)),
        );
    }

    #[test]
    fn typed_marks_convert_to_and_from_mark() {
        let mark: Mark = LineMark::new().x("a").into();
        assert_eq!(mark.kind, MarkKind::Line);
        let line = LineMark::from(mark.clone());
        assert!(line.channels.x.is_some());
        assert_eq!(line.into_mark().kind, MarkKind::Line);
        // `from_mark` keeps the mark it is given, whatever its kind.
        let wrapped = DotMark::from_mark(Mark::new(MarkKind::Text).y("b"));
        assert_eq!(wrapped.0.kind, MarkKind::Text);
        assert!(Mark::from(wrapped.clone()).channels.y.is_some());
        assert!(wrapped.into_mark().channels.y.is_some());
    }

    #[test]
    fn typed_marks_deref_to_their_mark() {
        let bar = BarYMark::new().y("value").fill_opacity(0.5);
        let mark: &Mark = &bar;
        assert!(std::ptr::eq(mark, &bar.0));
        assert!(bar.channels.y.is_some());
        assert_eq!(bar.style.fill_opacity, 0.5);
    }
}
