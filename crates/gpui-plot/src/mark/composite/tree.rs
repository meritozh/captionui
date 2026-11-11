//! Plot's tree mark: `expand_tree`, which draws a hierarchy as links, dots and labels.

use gpui::Hsla;

use crate::mark::{Curve, Mark, MarkKind, MarkStyle, TextAnchor};
use crate::style::ColorChannel;

/// Plot's `tree` (and `cluster`) mark as the marks it composes: a link per
/// relation (stroked `#777` at half opacity with a bump curve, or by
/// `node:internal`), a dot per node filled by `node:internal`, and the
/// node names, mirrored — leaves' labels after the node, internal nodes'
/// before it — with a halo.
pub fn expand_tree(mark: Mark, spec: crate::hierarchy::TreeSpec) -> Vec<Mark> {
    let data = mark.data.clone();
    let base = |kind: MarkKind| {
        let mut m = Mark::new(kind);
        m.data = data.clone();
        m.fx = mark.fx.clone();
        m.fy = mark.fy.clone();
        m
    };
    let mut link = base(MarkKind::Link).tree_link(spec.clone());
    // Plot's tree strokes its links by the fill, else by `node:internal`.
    link.channels.stroke = mark
        .channels
        .stroke
        .clone()
        .or_else(|| mark.channels.fill.clone())
        .or(Some(ColorChannel::Field("node:internal".into())));
    link.style.stroke_opacity = 0.5;
    link.style.stroke_width = if mark.style.stroke_width != MarkStyle::default().stroke_width {
        mark.style.stroke_width
    } else {
        1.5
    };
    if mark.style.curve != Curve::Linear {
        link.style.curve = mark.style.curve;
    }
    link.style.markers = mark.style.markers;
    let mut node = base(MarkKind::Dot).tree_node(spec.clone());
    node.channels.fill = mark
        .channels
        .fill
        .clone()
        .or(Some(ColorChannel::Field("node:internal".into())));
    let dx = if spec.right { -6. } else { 6. };
    let halo = mark.style.text_stroke.or(Some(Hsla::white()));
    let mut labels = Vec::new();
    // Plot's `textLayout`: "mirrored" for its tree and cluster layouts,
    // "normal" (every label after its node) for any other.
    let mirrored = spec.layout != crate::hierarchy::TreeLayout::Indent;
    let layouts: &[(Option<&str>, TextAnchor, f32)] = if mirrored {
        &[
            (Some("node:external"), TextAnchor::Start, dx),
            (Some("node:internal"), TextAnchor::End, -dx),
        ]
    } else {
        &[(None, TextAnchor::Start, dx)]
    };
    for &(filter, anchor, dx) in layouts {
        let mut text = base(MarkKind::Text).tree_node(spec.clone());
        if let Some(filter) = filter {
            text = text.tree_filter(filter);
        }
        text.channels.text = Some(crate::table::Channel::Field("node:name".into()));
        text.style.text_anchor = if spec.right {
            if anchor == TextAnchor::Start {
                TextAnchor::End
            } else {
                TextAnchor::Start
            }
        } else {
            anchor
        };
        text.style.text_anchor_set = true;
        text.style.dx = dx;
        text.style.text_stroke = halo.filter(|c| c.a > 0.);
        labels.push(text);
    }
    let mut out = vec![link];
    if mark.style.markers.iter().all(Option::is_none) {
        out.push(node);
    }
    out.extend(labels);
    out
}
