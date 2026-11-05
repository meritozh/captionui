//! The docs' Transforms section.

#[path = "bin.rs"]
pub mod bin;
#[path = "centroid.rs"]
pub mod centroid;
#[path = "dodge.rs"]
pub mod dodge;
#[path = "filter.rs"]
pub mod filter;
#[path = "group.rs"]
pub mod group;
#[path = "hexbin.rs"]
pub mod hexbin;
#[path = "interval.rs"]
pub mod interval;
#[path = "map.rs"]
pub mod map;
#[path = "normalize.rs"]
pub mod normalize;
#[path = "select.rs"]
pub mod select;
#[path = "shift.rs"]
pub mod shift;
#[path = "sort.rs"]
pub mod sort;
#[path = "stack.rs"]
pub mod stack;
#[path = "tree.rs"]
pub mod tree;
#[path = "window.rs"]
pub mod window;

/// This section's ported examples, page by page.
pub fn examples() -> Vec<super::Example> {
    let mut all = Vec::new();
    all.extend(bin::examples());
    all.extend(centroid::examples());
    all.extend(dodge::examples());
    all.extend(filter::examples());
    all.extend(group::examples());
    all.extend(hexbin::examples());
    all.extend(interval::examples());
    all.extend(map::examples());
    all.extend(normalize::examples());
    all.extend(select::examples());
    all.extend(shift::examples());
    all.extend(sort::examples());
    all.extend(stack::examples());
    all.extend(tree::examples());
    all.extend(window::examples());
    all
}
