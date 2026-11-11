//! Tests that span several stages of the pipeline.

/// The behaviour the pipeline's hot-loop rewrites must keep, each checked
/// against a naive reading of the raw records: group and bin order, bound
/// `$in:` inputs, box parts reading their value channel, facet partitions
/// shared only between alike marks, and a `z` shared with a position.
mod equivalence;

/// The transforms' composition order in [`prepare_mark`], pinned so a future
/// rearrangement cannot silently reorder them.
///
/// The pipeline is `select/reduce` → `map` → `shift` → `shuffle` (record
/// rewrites, run on the cloned `Dataset` before any row resolves), then
/// `sort` → `window` → `normalize` → `group` → `reduce` → `bin` → `stack` (row
/// rewrites, run on the resolved rows). Each test below exercises one edge of
/// that order: if a transform moved, an observable value changes.
mod transform_order;
