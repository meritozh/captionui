//! CSS colors and Plot's named schemes.
//!
//! Plot reads a mark's `fill: "red"` as a constant color and `fill: "sex"`
//! as a channel: a string that parses as a CSS color is a color. [`parse`]
//! is that test. [`scheme`] resolves Plot's scheme names (`"turbo"`,
//! `"BuRd"`, `"tableau10"`) to color ranges.

use gpui::Hsla;

use crate::scale::{ColorRange, hsla_from_hex, rgb_to_hsla};
use crate::schemes;

/// `currentColor` in a scale's `range`: a stand-in the color scale replaces
/// with the theme's foreground, since the options don't know the theme.
pub const CURRENT_COLOR: Hsla = Hsla {
    h: -1.,
    s: 0.,
    l: 0.,
    a: 1.,
};

/// A CSS color: a name, `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()` or
/// `hsl()`/`hsla()`. `transparent` and `none` are fully transparent.
pub fn parse(text: &str) -> Option<Hsla> {
    let text = text.trim();
    let lower = text.to_ascii_lowercase();
    if lower == "none" || lower == "transparent" {
        return Some(Hsla::transparent_black());
    }
    if let Some(hex) = lower.strip_prefix('#') {
        let digits: Vec<u32> = hex.chars().map(|c| c.to_digit(16)).collect::<Option<_>>()?;
        let channel = |a: u32, b: u32| (a * 16 + b) as f32 / 255.;
        return match digits.len() {
            3 => Some(rgb_to_hsla(
                channel(digits[0], digits[0]),
                channel(digits[1], digits[1]),
                channel(digits[2], digits[2]),
                1.,
            )),
            4 => Some(rgb_to_hsla(
                channel(digits[0], digits[0]),
                channel(digits[1], digits[1]),
                channel(digits[2], digits[2]),
                channel(digits[3], digits[3]),
            )),
            6 => Some(rgb_to_hsla(
                channel(digits[0], digits[1]),
                channel(digits[2], digits[3]),
                channel(digits[4], digits[5]),
                1.,
            )),
            8 => Some(rgb_to_hsla(
                channel(digits[0], digits[1]),
                channel(digits[2], digits[3]),
                channel(digits[4], digits[5]),
                channel(digits[6], digits[7]),
            )),
            _ => None,
        };
    }
    if let Some(args) = function(&lower, &["rgb", "rgba"]) {
        let value = |text: &str, scale: f32| -> Option<f32> {
            match text.strip_suffix('%') {
                Some(percent) => percent.trim().parse::<f32>().ok().map(|v| v / 100.),
                None => text.trim().parse::<f32>().ok().map(|v| v / scale),
            }
        };
        let r = value(args.first()?, 255.)?;
        let g = value(args.get(1)?, 255.)?;
        let b = value(args.get(2)?, 255.)?;
        let a = args.get(3).and_then(|a| value(a, 1.)).unwrap_or(1.);
        return Some(rgb_to_hsla(r, g, b, a));
    }
    if let Some(args) = function(&lower, &["hsl", "hsla"]) {
        let h = args
            .first()?
            .trim()
            .trim_end_matches("deg")
            .parse::<f32>()
            .ok()?;
        let percent = |text: &String| {
            text.trim()
                .trim_end_matches('%')
                .parse::<f32>()
                .ok()
                .map(|v| v / 100.)
        };
        let s = percent(args.get(1)?)?;
        let l = percent(args.get(2)?)?;
        let a = args
            .get(3)
            .and_then(|a| a.trim().parse::<f32>().ok())
            .unwrap_or(1.);
        return Some(Hsla {
            h: (h / 360.).rem_euclid(1.),
            s,
            l,
            a,
        });
    }
    NAMED
        .iter()
        .find(|(name, _)| *name == lower)
        .map(|(_, hex)| hsla_from_hex(*hex))
}

/// The arguments of `name(…)`, split on commas, spaces or `/`.
fn function(text: &str, names: &[&str]) -> Option<Vec<String>> {
    let open = text.find('(')?;
    if !names.contains(&&text[..open]) || !text.ends_with(')') {
        return None;
    }
    Some(
        text[open + 1..text.len() - 1]
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
    )
}

/// Plot's name for a reversed scheme: `"BuRd"` is `"RdBu"` read backwards.
const REVERSED: &[(&str, &str)] = &[("burd", "rdbu"), ("buylrd", "rdylbu")];

/// A named scheme as a color range: a continuous ramp for the sequential
/// and diverging schemes (diverging ones pivot at their middle), a palette
/// for the categorical ones. Names are case-insensitive, as Plot's are.
pub fn scheme(name: &str) -> Option<ColorRange> {
    let lower = name.to_ascii_lowercase();
    let (lookup, reversed) = match REVERSED.iter().find(|(alias, _)| *alias == lower) {
        Some((_, target)) => (target.to_string(), true),
        None => (lower, false),
    };
    if let Some((_, colors)) = schemes::CATEGORICAL
        .iter()
        .find(|(scheme, _)| scheme.to_ascii_lowercase() == lookup)
    {
        return Some(ColorRange::Ordinal(
            colors.iter().map(|hex| hsla_from_hex(*hex)).collect(),
        ));
    }
    let (_, _, stops) = schemes::CONTINUOUS
        .iter()
        .find(|(scheme, _, _)| scheme.to_ascii_lowercase() == lookup)?;
    let mut stops: Vec<Hsla> = stops.iter().map(|hex| hsla_from_hex(*hex)).collect();
    if reversed {
        stops.reverse();
    }
    Some(ColorRange::Continuous(stops))
}

/// Whether a named scheme is diverging, so a scale over it is centered on
/// its pivot (zero by default), as Plot's diverging scales are.
pub fn is_diverging(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    REVERSED.iter().any(|(alias, _)| *alias == lower)
        || schemes::CONTINUOUS
            .iter()
            .any(|(scheme, diverging, _)| *diverging && scheme.to_ascii_lowercase() == lower)
}

/// The `k` colors Plot gives an ordinal scale over a sequential or diverging
/// scheme: d3's discrete ColorBrewer scheme when it has one for `k`, else `k`
/// samples of the ramp.
pub fn discrete(name: &str, k: usize) -> Option<Vec<Hsla>> {
    let lower = name.to_ascii_lowercase();
    let (lookup, reversed) = match REVERSED.iter().find(|(alias, _)| *alias == lower) {
        Some((_, target)) => (target.to_string(), true),
        None => (lower, false),
    };
    // Plot's `ordinalRange`: below three values, ColorBrewer's three-color
    // scheme minus its middle (two values) or only its middle (one).
    if k < 3
        && let Some(three) = schemes::DISCRETE
            .iter()
            .find(|(scheme, _)| scheme.to_ascii_lowercase() == lookup)
            .and_then(|(_, sets)| sets.get(3))
            .filter(|set| set.len() == 3)
    {
        let pick: Vec<u32> = if k == 2 {
            vec![three[0], three[2]]
        } else {
            vec![three[1]]
        };
        let mut colors: Vec<Hsla> = pick.into_iter().map(hsla_from_hex).collect();
        if reversed {
            colors.reverse();
        }
        return Some(colors);
    }
    let mut colors = schemes::DISCRETE
        .iter()
        .find(|(scheme, _)| scheme.to_ascii_lowercase() == lookup)
        .and_then(|(_, sets)| sets.get(k))
        .filter(|set| !set.is_empty())
        .map(|set| {
            set.iter()
                .map(|hex| hsla_from_hex(*hex))
                .collect::<Vec<_>>()
        })
        .or_else(|| match scheme(&lookup)? {
            ColorRange::Continuous(stops) => Some(
                (0..k)
                    .map(|i| {
                        let t = if k <= 1 {
                            0.5
                        } else {
                            i as f32 / (k - 1) as f32
                        };
                        crate::scale::interpolate_stops(&stops, t)
                    })
                    .collect(),
            ),
            ColorRange::Ordinal(colors) => Some(colors),
            ColorRange::Diverging(..) => None,
        })?;
    if reversed {
        colors.reverse();
    }
    Some(colors)
}

/// CSS's named colors, as d3-color reads them.
pub static NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xf0f8ff),
    ("antiquewhite", 0xfaebd7),
    ("aqua", 0x00ffff),
    ("aquamarine", 0x7fffd4),
    ("azure", 0xf0ffff),
    ("beige", 0xf5f5dc),
    ("bisque", 0xffe4c4),
    ("black", 0x000000),
    ("blanchedalmond", 0xffebcd),
    ("blue", 0x0000ff),
    ("blueviolet", 0x8a2be2),
    ("brown", 0xa52a2a),
    ("burlywood", 0xdeb887),
    ("cadetblue", 0x5f9ea0),
    ("chartreuse", 0x7fff00),
    ("chocolate", 0xd2691e),
    ("coral", 0xff7f50),
    ("cornflowerblue", 0x6495ed),
    ("cornsilk", 0xfff8dc),
    ("crimson", 0xdc143c),
    ("cyan", 0x00ffff),
    ("darkblue", 0x00008b),
    ("darkcyan", 0x008b8b),
    ("darkgoldenrod", 0xb8860b),
    ("darkgray", 0xa9a9a9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xa9a9a9),
    ("darkkhaki", 0xbdb76b),
    ("darkmagenta", 0x8b008b),
    ("darkolivegreen", 0x556b2f),
    ("darkorange", 0xff8c00),
    ("darkorchid", 0x9932cc),
    ("darkred", 0x8b0000),
    ("darksalmon", 0xe9967a),
    ("darkseagreen", 0x8fbc8f),
    ("darkslateblue", 0x483d8b),
    ("darkslategray", 0x2f4f4f),
    ("darkslategrey", 0x2f4f4f),
    ("darkturquoise", 0x00ced1),
    ("darkviolet", 0x9400d3),
    ("deeppink", 0xff1493),
    ("deepskyblue", 0x00bfff),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1e90ff),
    ("firebrick", 0xb22222),
    ("floralwhite", 0xfffaf0),
    ("forestgreen", 0x228b22),
    ("fuchsia", 0xff00ff),
    ("gainsboro", 0xdcdcdc),
    ("ghostwhite", 0xf8f8ff),
    ("gold", 0xffd700),
    ("goldenrod", 0xdaa520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xadff2f),
    ("grey", 0x808080),
    ("honeydew", 0xf0fff0),
    ("hotpink", 0xff69b4),
    ("indianred", 0xcd5c5c),
    ("indigo", 0x4b0082),
    ("ivory", 0xfffff0),
    ("khaki", 0xf0e68c),
    ("lavender", 0xe6e6fa),
    ("lavenderblush", 0xfff0f5),
    ("lawngreen", 0x7cfc00),
    ("lemonchiffon", 0xfffacd),
    ("lightblue", 0xadd8e6),
    ("lightcoral", 0xf08080),
    ("lightcyan", 0xe0ffff),
    ("lightgoldenrodyellow", 0xfafad2),
    ("lightgray", 0xd3d3d3),
    ("lightgreen", 0x90ee90),
    ("lightgrey", 0xd3d3d3),
    ("lightpink", 0xffb6c1),
    ("lightsalmon", 0xffa07a),
    ("lightseagreen", 0x20b2aa),
    ("lightskyblue", 0x87cefa),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xb0c4de),
    ("lightyellow", 0xffffe0),
    ("lime", 0x00ff00),
    ("limegreen", 0x32cd32),
    ("linen", 0xfaf0e6),
    ("magenta", 0xff00ff),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66cdaa),
    ("mediumblue", 0x0000cd),
    ("mediumorchid", 0xba55d3),
    ("mediumpurple", 0x9370db),
    ("mediumseagreen", 0x3cb371),
    ("mediumslateblue", 0x7b68ee),
    ("mediumspringgreen", 0x00fa9a),
    ("mediumturquoise", 0x48d1cc),
    ("mediumvioletred", 0xc71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xf5fffa),
    ("mistyrose", 0xffe4e1),
    ("moccasin", 0xffe4b5),
    ("navajowhite", 0xffdead),
    ("navy", 0x000080),
    ("oldlace", 0xfdf5e6),
    ("olive", 0x808000),
    ("olivedrab", 0x6b8e23),
    ("orange", 0xffa500),
    ("orangered", 0xff4500),
    ("orchid", 0xda70d6),
    ("palegoldenrod", 0xeee8aa),
    ("palegreen", 0x98fb98),
    ("paleturquoise", 0xafeeee),
    ("palevioletred", 0xdb7093),
    ("papayawhip", 0xffefd5),
    ("peachpuff", 0xffdab9),
    ("peru", 0xcd853f),
    ("pink", 0xffc0cb),
    ("plum", 0xdda0dd),
    ("powderblue", 0xb0e0e6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xff0000),
    ("rosybrown", 0xbc8f8f),
    ("royalblue", 0x4169e1),
    ("saddlebrown", 0x8b4513),
    ("salmon", 0xfa8072),
    ("sandybrown", 0xf4a460),
    ("seagreen", 0x2e8b57),
    ("seashell", 0xfff5ee),
    ("sienna", 0xa0522d),
    ("silver", 0xc0c0c0),
    ("skyblue", 0x87ceeb),
    ("slateblue", 0x6a5acd),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xfffafa),
    ("springgreen", 0x00ff7f),
    ("steelblue", 0x4682b4),
    ("tan", 0xd2b48c),
    ("teal", 0x008080),
    ("thistle", 0xd8bfd8),
    ("tomato", 0xff6347),
    ("turquoise", 0x40e0d0),
    ("violet", 0xee82ee),
    ("wheat", 0xf5deb3),
    ("white", 0xffffff),
    ("whitesmoke", 0xf5f5f5),
    ("yellow", 0xffff00),
    ("yellowgreen", 0x9acd32),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_colors_parse() {
        assert!(parse("red").is_some());
        assert!(parse("#f00").is_some());
        assert!(parse("rgb(255, 0, 0)").is_some());
        assert!(parse("steelblue").is_some());
        assert!(parse("sex").is_none());
        assert!(parse("Anomaly").is_none());
        assert_eq!(parse("none").unwrap().a, 0.);
    }

    #[test]
    fn schemes_resolve_by_any_case() {
        assert!(matches!(scheme("turbo"), Some(ColorRange::Continuous(_))));
        assert!(matches!(scheme("Tableau10"), Some(ColorRange::Ordinal(_))));
        assert!(is_diverging("BuRd") && is_diverging("rdbu") && !is_diverging("viridis"));
        assert_eq!(discrete("spectral", 9).unwrap().len(), 9);
        assert_eq!(discrete("turbo", 4).unwrap().len(), 4);
    }
}
