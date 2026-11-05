//! Hierarchical layout for the `tree` mark: parse slash-separated paths into a
//! hierarchy and lay it out with a tidy tree algorithm.
//!
//! Plot's `tree` mark takes data as slash-separated paths — `"Chaos/Gaia/
//! Mountains"`, say — and draws a parent–child link per relation plus a node per
//! distinct path. This module turns that flat list into a positioned tree.

use std::collections::HashMap;

/// A node in the laid-out tree.
#[derive(Clone, Debug)]
pub struct Node {
    /// The node's own label, the last component of its path.
    pub label: String,
    /// The node's depth below the root, in levels.
    pub depth: usize,
    /// The source row that cited this node first, for the tooltip.
    pub row: usize,
    /// The laid-out position, alongside its depth level (`width` ↦ x slots).
    pub x: f64,
    /// The laid-out position down the levels.
    pub y: f64,
}

/// A laid-out tree: every node and every parent–child link between them.
#[derive(Clone, Debug)]
pub struct Tree {
    /// Every node, in first-encounter order.
    pub nodes: Vec<Node>,
    /// Every link, as `(child, parent)` indices into `nodes`.
    pub links: Vec<(usize, usize)>,
    /// The tree's width in slot units (one per leaf layering).
    pub width: f64,
    /// The tree's height in level units.
    pub height: f64,
}

/// Parse slash-separated paths into a hierarchy and lay it out tidily.
///
/// `paths` is one path per row of the source data, each a chain of labels from
/// the root down. A node appears once, at the row that named it first; every
/// parent–child relation between newly-seen nodes becomes a link.
///
/// The layout is the classic tidy tree, spacing equal-subtree siblings so each
/// node's descendants stand apart: a leaf takes the next x slot, a branch sits
/// at the mean of its children. `x` runs across the leaves (slot units), `y`
/// down the levels. The caller maps these onto the frame the way a density
/// estimate maps onto its axis.
pub fn tidy(paths: &[Vec<&str>]) -> Tree {
    // Build the hierarchy from first occurrences. `index` maps a node's joined
    // label chain to its node index; `parent` is that of each node.
    let mut nodes: Vec<Node> = Vec::new();
    let mut parents: Vec<Option<usize>> = Vec::new();
    let mut links: Vec<(usize, usize)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();

    // Every prefix of a path is a node too, as in Plot's stratify: `"a/b/c"`
    // implies `"a"` and `"a/b"`. Walking each path from its root creates a
    // parent before its child, so the hierarchy does not depend on whether a
    // parent's own row comes before or after its children's.
    for (row, path) in paths.iter().enumerate() {
        let mut parent: Option<usize> = None;
        for depth in 0..path.len() {
            let chain = link(&path[..=depth], '/');
            let node = match index.get(&chain) {
                Some(&node) => node,
                None => {
                    let node = nodes.len();
                    index.insert(chain, node);
                    nodes.push(Node {
                        label: path[depth].to_string(),
                        depth,
                        row,
                        x: 0.,
                        y: 0.,
                    });
                    parents.push(parent);
                    if let Some(p) = parent {
                        links.push((node, p));
                    }
                    node
                }
            };
            parent = Some(node);
        }
    }

    // Children per node, for the tidy pass.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    for &(child, parent) in &links {
        children[parent].push(child);
    }
    // Roots: nodes with no parent.
    let roots: Vec<usize> = (0..nodes.len()).filter(|i| parents[*i].is_none()).collect();

    // Post-order tidy placement. Returns the node's x slot; leaves consume the
    // next slot, branches centre over their children.
    let mut positions: Vec<f64> = vec![0.; nodes.len()];
    let mut depths: Vec<usize> = vec![0; nodes.len()];
    let mut next_leaf = 0usize;
    let mut max_depth = 0usize;
    fn place(
        node: usize,
        depth: usize,
        children: &[Vec<usize>],
        positions: &mut Vec<f64>,
        depths: &mut Vec<usize>,
        next_leaf: &mut usize,
        max_depth: &mut usize,
    ) -> f64 {
        *max_depth = (*max_depth).max(depth);
        depths[node] = depth;
        let subs = &children[node];
        if subs.is_empty() {
            let slot = *next_leaf;
            *next_leaf += 1;
            positions[node] = slot as f64;
            return slot as f64;
        }
        let mut total = 0.;
        for &child in subs {
            total += place(
                child,
                depth + 1,
                children,
                positions,
                depths,
                next_leaf,
                max_depth,
            );
        }
        let x = total / subs.len() as f64;
        positions[node] = x;
        x
    }
    for root in &roots {
        place(
            *root,
            0,
            &children,
            &mut positions,
            &mut depths,
            &mut next_leaf,
            &mut max_depth,
        );
    }

    for (node, node_pos) in positions.into_iter().enumerate() {
        nodes[node].x = node_pos;
        // The depth that matters is the *structural* one reached during
        // placement, because a node whose parent path was absent becomes a
        // visual root even though its own label says otherwise.
        nodes[node].depth = depths[node];
        nodes[node].y = depths[node] as f64;
    }
    let width = (next_leaf as f64).max(1.);
    let height = (max_depth as f64 + 1.).max(1.);

    Tree {
        nodes,
        links,
        width,
        height,
    }
}

/// Join a path's labels with `/`, the way `tidy` keys nodes by their chain.
fn link(path: &[&str], delimiter: char) -> String {
    let mut chain = String::new();
    for (i, part) in path.iter().enumerate() {
        if i > 0 {
            chain.push(delimiter);
        }
        chain.push_str(part);
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_paths_into_nodes_and_links() {
        let paths = ["a", "a/b", "a/c", "a/b/d"]
            .into_iter()
            .map(|p| p.split('/').collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let tree = tidy(&paths);
        // The four distinct paths are four nodes; "a/b/d" adds only d.
        assert_eq!(tree.nodes.len(), 4);
        let labels: Vec<&str> = tree.nodes.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, ["a", "b", "c", "d"]);
        assert_eq!(tree.links.len(), 3);
        assert_eq!(tree.height, 3.);
    }

    #[test]
    fn lays_leaves_apart_and_centres_branches() {
        let paths = ["r/a", "r/b", "r/c", "r/a/x", "r/a/y"]
            .into_iter()
            .map(|p| p.split('/').collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let tree = tidy(&paths);
        // `r` is implied by every path, so it is the one root (Plot's
        // stratify). Node order: r(0), a(1), b(2), c(3), x(4), y(5).
        let labels: Vec<&str> = tree.nodes.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, ["r", "a", "b", "c", "x", "y"]);
        // a's children x and y take slots 0 and 1, so a sits at 0.5; the
        // leaves b and c take slots 2 and 3; r centres over its children.
        assert!((tree.nodes[1].x - 0.5).abs() < 1e-9);
        assert_eq!(tree.nodes[2].x, 2.);
        assert_eq!(tree.nodes[3].x, 3.);
        assert_eq!(tree.nodes[4].x, 0.);
        assert_eq!(tree.nodes[5].x, 1.);
        assert!((tree.nodes[0].x - (0.5 + 2. + 3.) / 3.).abs() < 1e-9);
        assert_eq!(tree.nodes[0].y, 0.);
        assert_eq!(tree.nodes[1].y, 1.);
        assert_eq!(tree.nodes[4].y, 2.);
        assert_eq!(tree.width, 4.);
        assert_eq!(tree.height, 3.);
    }

    #[test]
    fn a_child_before_its_parent_still_links() {
        let paths = [vec!["a", "b"], vec!["a"]];
        let tree = tidy(&paths);
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.links, vec![(1, 0)]);
        assert_eq!(tree.nodes[1].depth, 1);
    }
}
