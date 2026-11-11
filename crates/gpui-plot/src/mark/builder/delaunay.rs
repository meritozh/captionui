//! The typed builders of the Delaunay marks: links, meshes, hull and Voronoi.

use gpui::SharedString;

use super::mark_builder;
use crate::mark::{
    BlendMode, Curve, DomainSort, FrameAnchor, Interval, LineAnchor, MapReducer, Mark, MarkKind,
    TextAnchor, VectorAnchor,
};
use crate::style::{ColorChannel, NumberChannel};
use crate::table::IntoChannel;
use crate::transform::{
    Anchor, Dodge, GroupReducer, Mapper, Reducer, Selector, StackOffset, Thresholds, WindowReducer,
};

mark_builder!(
    /// A Delaunay triangulation drawn as links, Plot's `delaunayLink`.
    DelaunayLinkMark,
    MarkKind::DelaunayLink
);
mark_builder!(
    /// A Delaunay triangulation drawn as one mesh, Plot's `delaunayMesh`.
    DelaunayMeshMark,
    MarkKind::DelaunayMesh
);
mark_builder!(
    /// The convex hull of the data, Plot's `hull`.
    HullMark,
    MarkKind::Hull
);
mark_builder!(
    /// A Voronoi cell per datum, Plot's `voronoi`.
    VoronoiMark,
    MarkKind::Voronoi
);
mark_builder!(
    /// The Voronoi mesh of the data, Plot's `voronoiMesh`.
    VoronoiMeshMark,
    MarkKind::VoronoiMesh
);
