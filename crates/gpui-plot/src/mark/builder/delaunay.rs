//! The typed builders of the Delaunay marks: links, meshes, hull and Voronoi.

use super::typed_marks;

typed_marks! {
    /// A Delaunay triangulation drawn as links, Plot's `delaunayLink`.
    DelaunayLinkMark = DelaunayLink;
    /// A Delaunay triangulation drawn as one mesh, Plot's `delaunayMesh`.
    DelaunayMeshMark = DelaunayMesh;
    /// The convex hull of the data, Plot's `hull`.
    HullMark = Hull;
    /// A Voronoi cell per datum, Plot's `voronoi`.
    VoronoiMark = Voronoi;
    /// The Voronoi mesh of the data, Plot's `voronoiMesh`.
    VoronoiMeshMark = VoronoiMesh;
}
