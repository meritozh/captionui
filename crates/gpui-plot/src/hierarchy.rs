//! Plot's tree transforms, `treeNode` and `treeLink`, and the d3-hierarchy
//! layouts behind them: `d3.tree` (Reingold–Tilford tidy trees, Buchheim's
//! linear-time variant) and `d3.cluster` (dendrograms), each with
//! `nodeSize([1, 1])` as Plot sets it, and an indented layout.
//!
//! The data are records with a delimited path (`"Chaos/Gaia/Mountains"`);
//! `d3.stratify().path(…)` builds the hierarchy, adding the implied interior
//! nodes. With Plot's left anchor a node's depth runs along x and its breadth
//! down y (y is the negated breadth).

use std::collections::HashMap;

use crate::table::{Dataset, Table};
use crate::value::Value;

/// Which d3-hierarchy layout positions the nodes, Plot's `treeLayout`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeLayout {
    /// `d3.tree`, a tidy tree.
    #[default]
    Tree,
    /// `d3.cluster`, leaves aligned at the deepest level.
    Cluster,
    /// The docs' `indent` layout: one row per node in preorder, indented by
    /// depth.
    Indent,
}

/// How siblings are ordered before layout, Plot's `treeSort`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeSort {
    /// By `node:height` (ascending), Plot's `"node:height"`.
    Height,
    /// By name.
    Name,
}

/// The options shared by `treeNode` and `treeLink`.
#[derive(Clone, Debug)]
pub struct TreeSpec {
    /// The field holding the delimited path (Plot's `path`, the datum itself
    /// for plain values by default).
    pub path: Option<gpui::SharedString>,
    pub delimiter: char,
    pub layout: TreeLayout,
    pub sort: Option<TreeSort>,
    /// Plot's `treeAnchor: "right"` mirrors the layout.
    pub right: bool,
}

impl Default for TreeSpec {
    fn default() -> Self {
        Self {
            path: None,
            delimiter: '/',
            layout: TreeLayout::Tree,
            sort: None,
            right: false,
        }
    }
}

#[derive(Clone, Debug)]
struct Node {
    name: String,
    path: String,
    parent: Option<usize>,
    children: Vec<usize>,
    depth: usize,
    height: usize,
    /// The source record, for the nodes the data names.
    data: Option<Table>,
    x: f64,
    y: f64,
}

/// A laid-out hierarchy.
pub struct Hierarchy {
    nodes: Vec<Node>,
    root: usize,
}

/// Plot's path normalizer: a leading delimiter is implied, a trailing one
/// dropped, and an escaped delimiter kept literal.
fn split_path(path: &str, delimiter: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut chars = path.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                current.push(next);
            }
        } else if c == delimiter {
            parts.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
    }
    parts.push(current);
    parts.into_iter().filter(|p| !p.is_empty()).collect()
}

impl Hierarchy {
    /// `d3.stratify().path(…)` over the records' paths.
    ///
    /// As d3 does, the records come first, in order, and the implied
    /// interior nodes after them, in the order their children name them; a
    /// node's children follow that order, so an implied parent sorts after
    /// its explicit siblings. The root is the topmost implied node, skipping
    /// implied nodes with a single child.
    pub fn stratify(data: &Dataset, spec: &TreeSpec) -> Option<Self> {
        let read = |row: &Table| -> Option<String> {
            let value = match &spec.path {
                Some(field) => row.value(field),
                None => row.value(crate::table::VALUE),
            };
            (!value.is_null()).then(|| value.label().to_string())
        };
        let mut keys: Vec<Vec<String>> = Vec::new();
        let mut records: Vec<Option<Table>> = Vec::new();
        for row in data.rows() {
            let Some(path) = read(row) else { continue };
            keys.push(split_path(&path, spec.delimiter));
            records.push(Some(row.clone()));
        }
        if keys.is_empty() {
            return None;
        }
        let mut seen: std::collections::HashSet<Vec<String>> = keys.iter().cloned().collect();
        let mut i = 0;
        while i < keys.len() {
            if let Some((_, parent)) = keys[i].split_last()
                && seen.insert(parent.to_vec())
            {
                keys.push(parent.to_vec());
                records.push(None);
            }
            i += 1;
        }
        let mut nodes: Vec<Node> = Vec::new();
        let mut index: HashMap<Vec<String>, usize> = HashMap::new();
        let mut ids = Vec::with_capacity(keys.len());
        for (parts, data) in keys.iter().zip(records) {
            if let Some(&id) = index.get(parts) {
                ids.push(None);
                if nodes[id].data.is_none() {
                    nodes[id].data = data;
                }
                continue;
            }
            index.insert(parts.clone(), nodes.len());
            ids.push(Some(nodes.len()));
            nodes.push(Node {
                name: parts.last().cloned().unwrap_or_default(),
                // Plot's `node:path` is d3's id: the slash-delimited path.
                path: format!("/{}", parts.join("/")),
                parent: None,
                children: Vec::new(),
                depth: 0,
                height: 0,
                data,
                x: 0.,
                y: 0.,
            });
        }
        for (parts, id) in keys.iter().zip(ids) {
            let (Some(id), Some((_, parent))) = (id, parts.split_last()) else {
                continue;
            };
            let parent = index[parent];
            nodes[id].parent = Some(parent);
            nodes[parent].children.push(id);
        }
        let mut root = *index.get(&Vec::new())?;
        while nodes[root].data.is_none() && nodes[root].children.len() == 1 {
            root = nodes[root].children[0];
        }
        nodes[root].parent = None;
        let mut h = Self { nodes, root };
        h.recompute_depths();
        Some(h)
    }

    fn recompute_depths(&mut self) {
        let mut stack = vec![(self.root, 0usize)];
        while let Some((id, depth)) = stack.pop() {
            self.nodes[id].depth = depth;
            for &c in &self.nodes[id].children.clone() {
                stack.push((c, depth + 1));
            }
        }
        // Heights, bottom-up.
        let order = self.preorder();
        for &id in order.iter().rev() {
            let h = self.nodes[id]
                .children
                .iter()
                .map(|c| self.nodes[*c].height + 1)
                .max()
                .unwrap_or(0);
            self.nodes[id].height = h;
        }
    }

    fn preorder(&self) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack = vec![self.root];
        while let Some(id) = stack.pop() {
            out.push(id);
            for &c in self.nodes[id].children.iter().rev() {
                stack.push(c);
            }
        }
        out
    }

    /// Sort siblings, d3's `node.sort`.
    pub fn sort(&mut self, by: TreeSort) {
        for id in 0..self.nodes.len() {
            let mut children = self.nodes[id].children.clone();
            match by {
                TreeSort::Height => children.sort_by_key(|c| self.nodes[*c].height),
                TreeSort::Name => {
                    children.sort_by(|a, b| self.nodes[*a].name.cmp(&self.nodes[*b].name))
                }
            }
            self.nodes[id].children = children;
        }
    }

    /// Lay the hierarchy out with `nodeSize([1, 1])`.
    pub fn layout(&mut self, layout: TreeLayout) {
        match layout {
            TreeLayout::Tree => self.tidy(),
            TreeLayout::Cluster => self.cluster(),
            TreeLayout::Indent => {
                for (i, id) in self.preorder().into_iter().enumerate() {
                    self.nodes[id].x = i as f64;
                    self.nodes[id].y = self.nodes[id].depth as f64;
                }
            }
        }
    }

    /// d3's `cluster`: leaves in order, one apart from a sibling and two from
    /// a cousin, parents centred over their children, every leaf at the
    /// deepest level.
    fn cluster(&mut self) {
        let mut previous: Option<usize> = None;
        let mut x = 0.0_f64;
        let max_depth = self.nodes.iter().map(|n| n.depth).max().unwrap_or(0);
        // Post-order: leaves left to right.
        fn post(h: &Hierarchy, id: usize, out: &mut Vec<usize>) {
            for &c in &h.nodes[id].children {
                post(h, c, out);
            }
            out.push(id);
        }
        let mut postorder = Vec::new();
        post(self, self.root, &mut postorder);
        for id in postorder {
            if self.nodes[id].children.is_empty() {
                if let Some(p) = previous {
                    x += if self.nodes[p].parent == self.nodes[id].parent {
                        1.
                    } else {
                        2.
                    };
                }
                self.nodes[id].x = x;
                self.nodes[id].y = max_depth as f64;
                previous = Some(id);
            } else {
                let c = self.nodes[id].children.clone();
                let mean = c.iter().map(|k| self.nodes[*k].x).sum::<f64>() / c.len() as f64;
                let deepest = c
                    .iter()
                    .map(|k| self.nodes[*k].y)
                    .fold(f64::INFINITY, f64::min);
                self.nodes[id].x = mean;
                self.nodes[id].y = deepest - 1.;
            }
        }
    }

    /// d3's `tree` (Buchheim et al.), separation 1 for siblings and 2 for
    /// cousins, nodeSize [1, 1].
    fn tidy(&mut self) {
        let n = self.nodes.len();
        let mut prelim = vec![0.0_f64; n];
        let mut modifier = vec![0.0_f64; n];
        let mut shift = vec![0.0_f64; n];
        let mut change = vec![0.0_f64; n];
        let mut thread: Vec<Option<usize>> = vec![None; n];
        let mut ancestor: Vec<usize> = (0..n).collect();
        let mut number = vec![0usize; n];
        for id in 0..n {
            for (i, &c) in self.nodes[id].children.iter().enumerate() {
                number[c] = i;
            }
        }
        let separation = |a: usize, b: usize, nodes: &[Node]| {
            if nodes[a].parent == nodes[b].parent {
                1.
            } else {
                2.
            }
        };
        let nodes = self.nodes.clone();
        let left_sibling = |v: usize| -> Option<usize> {
            let p = nodes[v].parent?;
            let i = number[v];
            (i > 0).then(|| nodes[p].children[i - 1])
        };
        let leftmost_sibling = |v: usize| -> Option<usize> {
            let p = nodes[v].parent?;
            let first = nodes[p].children[0];
            (first != v).then_some(first)
        };
        let next_left =
            |v: usize, thread: &[Option<usize>]| nodes[v].children.first().copied().or(thread[v]);
        let next_right =
            |v: usize, thread: &[Option<usize>]| nodes[v].children.last().copied().or(thread[v]);

        // Post-order first walk.
        fn post(nodes: &[Node], id: usize, out: &mut Vec<usize>) {
            for &c in &nodes[id].children {
                post(nodes, c, out);
            }
            out.push(id);
        }
        let mut order = Vec::new();
        post(&nodes, self.root, &mut order);
        for &v in &order {
            if nodes[v].children.is_empty() {
                prelim[v] = left_sibling(v).map_or(0., |w| prelim[w] + separation(v, w, &nodes));
                continue;
            }
            // Apportion each child.
            let mut default_ancestor = nodes[v].children[0];
            for &w in &nodes[v].children {
                if let Some(left) = left_sibling(w) {
                    let (mut vip, mut vop) = (w, w);
                    let mut vim = left;
                    let mut vom = leftmost_sibling(vip).unwrap_or(vip);
                    let (mut sip, mut sop, mut sim, mut som) =
                        (modifier[vip], modifier[vop], modifier[vim], modifier[vom]);
                    while let (Some(nr), Some(nl)) =
                        (next_right(vim, &thread), next_left(vip, &thread))
                    {
                        vim = nr;
                        vip = nl;
                        vom = next_left(vom, &thread).unwrap_or(vom);
                        vop = next_right(vop, &thread).unwrap_or(vop);
                        ancestor[vop] = w;
                        let shift_by =
                            prelim[vim] + sim - (prelim[vip] + sip) + separation(vim, vip, &nodes);
                        if shift_by > 0. {
                            let a = if nodes[ancestor[vim]].parent == nodes[w].parent {
                                ancestor[vim]
                            } else {
                                default_ancestor
                            };
                            let subtrees = (number[w] as f64 - number[a] as f64).max(1.);
                            change[w] -= shift_by / subtrees;
                            shift[w] += shift_by;
                            change[a] += shift_by / subtrees;
                            prelim[w] += shift_by;
                            modifier[w] += shift_by;
                            sip += shift_by;
                            sop += shift_by;
                        }
                        sim += modifier[vim];
                        sip += modifier[vip];
                        som += modifier[vom];
                        sop += modifier[vop];
                    }
                    if next_right(vim, &thread).is_some() && next_right(vop, &thread).is_none() {
                        thread[vop] = next_right(vim, &thread);
                        modifier[vop] += sim - sop;
                    }
                    if next_left(vip, &thread).is_some() && next_left(vom, &thread).is_none() {
                        thread[vom] = next_left(vip, &thread);
                        modifier[vom] += sip - som;
                        default_ancestor = w;
                    }
                }
            }
            // Execute shifts.
            let (mut s, mut c) = (0., 0.);
            for &w in nodes[v].children.iter().rev() {
                prelim[w] += s;
                modifier[w] += s;
                c += change[w];
                s += shift[w] + c;
            }
            let first = nodes[v].children[0];
            let last = *nodes[v].children.last().unwrap_or(&first);
            let mid = (prelim[first] + prelim[last]) / 2.;
            match left_sibling(v) {
                Some(w) => {
                    prelim[v] = prelim[w] + separation(v, w, &nodes);
                    modifier[v] = prelim[v] - mid;
                }
                None => prelim[v] = mid,
            }
        }
        // Second walk.
        let mut stack = vec![(self.root, 0.0_f64)];
        while let Some((v, m)) = stack.pop() {
            self.nodes[v].x = prelim[v] + m;
            self.nodes[v].y = self.nodes[v].depth as f64;
            for &c in &self.nodes[v].children {
                stack.push((c, m + modifier[v]));
            }
        }
    }

    fn place(&self, id: usize, spec: &TreeSpec) -> (f64, f64) {
        let n = &self.nodes[id];
        if spec.right {
            (-n.y, -n.x)
        } else {
            (n.y, -n.x)
        }
    }

    fn record(&self, id: usize) -> Table {
        let n = &self.nodes[id];
        n.data
            .clone()
            .unwrap_or_default()
            .with("node:name", Value::text(n.name.clone()))
            .with("node:path", Value::text(n.path.clone()))
            .with("node:internal", Value::Bool(!n.children.is_empty()))
            .with("node:external", Value::Bool(n.children.is_empty()))
            .with("node:depth", Value::Number(n.depth as f64))
            .with("node:height", Value::Number(n.height as f64))
    }

    /// Plot's `treeNode`: one record per node, with `x`/`y` its position and
    /// the `node:…` fields.
    pub fn node_records(&self, spec: &TreeSpec) -> Dataset {
        Dataset::new(self.preorder().into_iter().map(|id| {
            let (x, y) = self.place(id, spec);
            self.record(id)
                .with("$tree:x", Value::Number(x))
                .with("$tree:y", Value::Number(y))
        }))
    }

    /// Plot's `treeLink`: one record per parent → child link, with
    /// `x1`/`y1` the parent and `x2`/`y2` the child, and the child's
    /// `node:…` fields.
    pub fn link_records(&self, spec: &TreeSpec) -> Dataset {
        Dataset::new(self.preorder().into_iter().filter_map(|id| {
            let parent = self.nodes[id].parent?;
            let (x1, y1) = self.place(parent, spec);
            let (x2, y2) = self.place(id, spec);
            Some(
                self.record(id)
                    .with("$tree:x1", Value::Number(x1))
                    .with("$tree:y1", Value::Number(y1))
                    .with("$tree:x2", Value::Number(x2))
                    .with("$tree:y2", Value::Number(y2)),
            )
        }))
    }
}

/// Lay `data` out per `spec`, Plot's tree transforms' shared work.
pub fn layout(data: &Dataset, spec: &TreeSpec) -> Option<Hierarchy> {
    let mut h = Hierarchy::stratify(data, spec)?;
    if let Some(sort) = spec.sort {
        h.sort(sort);
    }
    h.layout(spec.layout);
    Some(h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tidy_tree_centres_parents() {
        let data = Dataset::values_of(
            [
                "Chaos/Gaia/Mountains",
                "Chaos/Gaia/Pontus",
                "Chaos/Eros",
                "Chaos/Erebus",
            ]
            .map(Value::text),
        );
        let h = layout(&data, &TreeSpec::default()).unwrap();
        let nodes = h.node_records(&TreeSpec::default());
        assert_eq!(nodes.len(), 6);
        let root = &nodes.rows()[0];
        assert_eq!(root.value("node:name"), Value::text("Chaos"));
        assert_eq!(root.number("$tree:x"), Some(0.));
        assert_eq!(h.link_records(&TreeSpec::default()).len(), 5);
    }
}
