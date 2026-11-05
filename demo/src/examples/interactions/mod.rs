//! The docs' Interactions section.

#[path = "crosshair.rs"]
pub mod crosshair;
#[path = "pointer.rs"]
pub mod pointer;

/// This section's ported examples, page by page.
pub fn examples() -> Vec<super::Example> {
    let mut all = Vec::new();
    all.extend(crosshair::examples());
    all.extend(pointer::examples());
    all
}
