//! `StyleProperty` -> React Native `StyleSheet` property/value text.
//!
//! Several platform differences from `dowel_web::css` that aren't just
//! "different syntax for the same idea":
//! - RN style values are unitless numbers (density-independent pixels), not
//!   CSS length strings -- no `px` suffix.
//! - RN's `flex` is a single number (roughly flex-grow), not a CSS
//!   `grow shrink basis` shorthand string -- `FlexShorthand::Auto`/
//!   `Initial`/`None` have no single-number equivalent, so they expand to
//!   `flexGrow`/`flexShrink` pairs instead.
//! - RN's `fontWeight` is a *string* (`'700'`), not a number, unlike CSS.
//! - `Color` resolves against the default Tailwind palette via
//!   `dowel_ir::resolve_color_token` (hex, since RN's style system doesn't
//!   understand `oklch()`). A token outside the default palette (custom
//!   theme colors, arbitrary values -- still proposal §16/Phase 4 territory)
//!   falls back to a placeholder marker string, deliberately not
//!   real-color-shaped so a missed resolution fails loudly instead of
//!   rendering a plausible-but-wrong color. RN has nothing like a CSS custom
//!   property to defer to the way Web's `var(--dowel-color-x)` does.

use dowel_ir::{
    Align, AlignSelf, BorderStyle, Color, Dimension, Display, FlexDirection, FlexShorthand, Justify,
    Length, LineHeight, Overflow, Position, Radius, StyleProperty, TextAlign, TextTransform,
};

fn radius_number(radius: &Radius) -> String {
    match radius {
        Radius::Length(l) => number(*l),
        // RN has no infinity. Any radius past half the box's shorter side
        // already renders as a pill, so a large finite value is the
        // standard way to express this -- the approximation is forced here,
        // unlike on Web.
        Radius::Full => "9999".to_string(),
    }
}

fn number(length: Length) -> String {
    let Length::Px(value) = length;
    format!("{value}")
}

fn dimension_value(dim: Dimension) -> String {
    match dim {
        Dimension::Length(length) => number(length),
        Dimension::Percent(pct) => format!("'{pct}%'"),
        Dimension::Auto => "'auto'".to_string(),
        // Refused upstream by `StyleProperty::unsupported_on_native`, which
        // fails the build. Nothing is emitted here so a build that swallowed
        // that error still can't ship a value RN would reject.
        Dimension::ViewportWidth(_) | Dimension::ViewportHeight(_) => String::new(),
    }
}

/// Builds React Native's combined `transform` array from whichever
/// standalone transform properties a rule carries, or `None` if it carries
/// none. Ordered translate -> rotate -> scale, matching how CSS applies its
/// standalone properties, so the two platforms compose identically.
pub fn transform_entry(props: &[StyleProperty]) -> Option<(&'static str, String)> {
    let mut parts: Vec<String> = Vec::new();
    for prop in props {
        if let StyleProperty::TranslateX(Length::Px(v)) = prop {
            parts.push(format!("{{ translateX: {v} }}"));
        }
    }
    for prop in props {
        if let StyleProperty::TranslateY(Length::Px(v)) = prop {
            parts.push(format!("{{ translateY: {v} }}"));
        }
    }
    for prop in props {
        if let StyleProperty::Rotate(a) = prop {
            parts.push(format!("{{ rotate: '{}deg' }}", a.degrees));
        }
    }
    for prop in props {
        if let StyleProperty::Scale(s) = prop {
            parts.push(format!("{{ scale: {} }}", s / 100.0));
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(("transform", format!("[{}]", parts.join(", "))))
}

/// Joins whichever ring/shadow utilities a rule carries into one
/// `boxShadow` string, in the same layer order the Web backend uses so both
/// platforms stack them identically.
///
/// React Native 0.81 accepts a CSS-like string for `boxShadow`, so the
/// composed value is the same shape as the Web one -- only the quoting and
/// the unitless-number convention differ.
pub fn box_shadow_entry(props: &[StyleProperty]) -> Option<(&'static str, String)> {
    let ring = props.iter().find_map(|p| match p {
        StyleProperty::RingWidth(Length::Px(v)) => Some(*v),
        _ => None,
    });
    let inset_ring = props.iter().find_map(|p| match p {
        StyleProperty::InsetRingWidth(Length::Px(v)) => Some(*v),
        _ => None,
    });
    let ring_color = props.iter().find_map(|p| match p {
        StyleProperty::RingColor(c) => Some(c),
        _ => None,
    });
    let inset_ring_color = props.iter().find_map(|p| match p {
        StyleProperty::InsetRingColor(c) => Some(c),
        _ => None,
    });
    let shadow = props.iter().find_map(|p| match p {
        StyleProperty::BoxShadow(s) => Some(s.clone()),
        _ => None,
    });

    // Tailwind's default ring colour. Unquoted here because the whole
    // `boxShadow` value is one string.
    let paint = |c: Option<&Color>| {
        c.map_or("currentcolor".to_string(), |c| resolve_color(c).trim_matches('\'').to_string())
    };

    let mut layers: Vec<String> = Vec::new();
    if let Some(width) = inset_ring {
        layers.push(format!("inset 0 0 0 {width}px {}", paint(inset_ring_color)));
    }
    if let Some(width) = ring {
        layers.push(format!("0 0 0 {width}px {}", paint(ring_color)));
    }
    if let Some(shadow) = shadow {
        // See the Web backend: `shadow-none` clears the shadow layer, not
        // the ring beside it.
        if shadow != "none" {
            layers.push(shadow);
        } else if layers.is_empty() {
            return Some(("boxShadow", "'none'".to_string()));
        }
    }
    (!layers.is_empty()).then(|| ("boxShadow", format!("'{}'", layers.join(", "))))
}

fn justify_literal(justify: &Justify) -> String {
    match justify {
        Justify::Start => "'flex-start'",
        Justify::Center => "'center'",
        Justify::End => "'flex-end'",
        Justify::Between => "'space-between'",
        Justify::Around => "'space-around'",
        Justify::Evenly => "'space-evenly'",
    }
    .to_string()
}

fn border_style_literal(style: &BorderStyle) -> String {
    match style {
        BorderStyle::Solid => "'solid'",
        BorderStyle::Dashed => "'dashed'",
        BorderStyle::Dotted => "'dotted'",
        // RN has no 'none' border style; a zero width is how you hide one.
        BorderStyle::None => "'solid'",
    }
    .to_string()
}

/// Resolves against the default Tailwind palette where possible (see module
/// docs); otherwise falls back to a marker string deliberately not
/// real-color-shaped, so a missed resolution fails loudly instead of
/// rendering a plausible-but-wrong color.
fn resolve_color(color: &Color) -> String {
    let Color::Token(token) = color;
    match dowel_ir::resolve_color_token(token) {
        Some(resolved) => format!("'{}'", resolved.hex),
        None => format!("'dowel-unresolved:{token}'"),
    }
}

/// Maps one `StyleProperty` to one or more `(rn-style-key, value)` pairs
/// (plural because e.g. `FlexShorthand::Auto` has no single-number RN
/// equivalent and must expand to two keys).
pub fn property_and_value(prop: &StyleProperty) -> Vec<(&'static str, String)> {
    match prop {
        StyleProperty::Display(d) => match d {
            Display::Flex => vec![("display", "'flex'".to_string())],
            Display::None => vec![("display", "'none'".to_string())],
            Display::Contents => vec![("display", "'contents'".to_string())],
            // No RN equivalent (Yoga has only the three above). The caller
            // raises `WebOnlyPropertyOnNative` and fails the build; nothing
            // is emitted here so a build that ignored the error can't ship
            // an invalid style value either.
            Display::Block | Display::InlineFlex | Display::Grid => Vec::new(),
        },
        StyleProperty::FlexDirection(dir) => vec![(
            "flexDirection",
            match dir {
                FlexDirection::Row => "'row'",
                FlexDirection::Column => "'column'",
                FlexDirection::RowReverse => "'row-reverse'",
                FlexDirection::ColumnReverse => "'column-reverse'",
            }
            .to_string(),
        )],
        StyleProperty::Flex(shorthand) => match shorthand {
            FlexShorthand::Grow(n) => vec![("flex", format!("{n}"))],
            FlexShorthand::Auto => vec![("flexGrow", "1".to_string()), ("flexShrink", "1".to_string())],
            FlexShorthand::Initial => vec![("flexGrow", "0".to_string()), ("flexShrink", "1".to_string())],
            FlexShorthand::None => vec![("flexGrow", "0".to_string()), ("flexShrink", "0".to_string())],
        },
        StyleProperty::AlignItems(align) => vec![(
            "alignItems",
            match align {
                Align::Start => "'flex-start'",
                Align::Center => "'center'",
                Align::End => "'flex-end'",
                Align::Stretch => "'stretch'",
                Align::Baseline => "'baseline'",
            }
            .to_string(),
        )],
        StyleProperty::AlignSelf(align) => vec![(
            "alignSelf",
            match align {
                AlignSelf::Auto => "'auto'",
                AlignSelf::Start => "'flex-start'",
                AlignSelf::Center => "'center'",
                AlignSelf::End => "'flex-end'",
                AlignSelf::Stretch => "'stretch'",
                AlignSelf::Baseline => "'baseline'",
            }
            .to_string(),
        )],
        StyleProperty::AlignContent(justify) => vec![("alignContent", justify_literal(justify))],
        StyleProperty::JustifyContent(justify) => vec![("justifyContent", justify_literal(justify))],
        StyleProperty::Gap(l) => vec![("gap", number(*l))],
        StyleProperty::RowGap(l) => vec![("rowGap", number(*l))],
        StyleProperty::ColumnGap(l) => vec![("columnGap", number(*l))],
        StyleProperty::MarginTop(d) => vec![("marginTop", dimension_value(*d))],
        StyleProperty::MarginRight(d) => vec![("marginRight", dimension_value(*d))],
        StyleProperty::MarginBottom(d) => vec![("marginBottom", dimension_value(*d))],
        StyleProperty::MarginLeft(d) => vec![("marginLeft", dimension_value(*d))],
        StyleProperty::PaddingTop(l) => vec![("paddingTop", number(*l))],
        StyleProperty::PaddingRight(l) => vec![("paddingRight", number(*l))],
        StyleProperty::PaddingBottom(l) => vec![("paddingBottom", number(*l))],
        StyleProperty::PaddingLeft(l) => vec![("paddingLeft", number(*l))],
        // RN's own direction-relative props; they resolve against
        // `I18nManager.isRTL` at runtime, same role as CSS's inline-start/end.
        StyleProperty::MarginInlineStart(d) => vec![("marginStart", dimension_value(*d))],
        StyleProperty::MarginInlineEnd(d) => vec![("marginEnd", dimension_value(*d))],
        StyleProperty::PaddingInlineStart(l) => vec![("paddingStart", number(*l))],
        StyleProperty::PaddingInlineEnd(l) => vec![("paddingEnd", number(*l))],
        StyleProperty::Width(d) => vec![("width", dimension_value(*d))],
        StyleProperty::Height(d) => vec![("height", dimension_value(*d))],
        StyleProperty::MinWidth(d) => vec![("minWidth", dimension_value(*d))],
        StyleProperty::MinHeight(d) => vec![("minHeight", dimension_value(*d))],
        StyleProperty::MaxWidth(d) => vec![("maxWidth", dimension_value(*d))],
        StyleProperty::MaxHeight(d) => vec![("maxHeight", dimension_value(*d))],
        StyleProperty::ZIndex(z) => vec![("zIndex", format!("{z}"))],
        StyleProperty::Position(pos) => vec![(
            "position",
            match pos {
                Position::Relative => "'relative'",
                Position::Absolute => "'absolute'",
            }
            .to_string(),
        )],
        StyleProperty::InsetTop(l) => vec![("top", number(*l))],
        StyleProperty::InsetRight(l) => vec![("right", number(*l))],
        StyleProperty::InsetBottom(l) => vec![("bottom", number(*l))],
        StyleProperty::InsetLeft(l) => vec![("left", number(*l))],
        StyleProperty::InsetInlineStart(l) => vec![("start", number(*l))],
        StyleProperty::InsetInlineEnd(l) => vec![("end", number(*l))],
        // No axis shorthand in React Native, so both edges are written.
        StyleProperty::InsetInline(l) => vec![("start", number(*l)), ("end", number(*l))],
        StyleProperty::InsetBlock(l) => vec![("top", number(*l)), ("bottom", number(*l))],
        // The block axis is only distinct from top/bottom under a vertical
        // `writing-mode`, which React Native has no concept of.
        StyleProperty::InsetBlockStart(l) => vec![("top", number(*l))],
        StyleProperty::InsetBlockEnd(l) => vec![("bottom", number(*l))],
        StyleProperty::BackgroundColor(c) => vec![("backgroundColor", resolve_color(c))],
        StyleProperty::Opacity(o) => vec![("opacity", format!("{o}"))],
        StyleProperty::BorderColor(c) => vec![("borderColor", resolve_color(c))],
        StyleProperty::BorderTopColor(c) => vec![("borderTopColor", resolve_color(c))],
        StyleProperty::BorderRightColor(c) => vec![("borderRightColor", resolve_color(c))],
        StyleProperty::BorderBottomColor(c) => vec![("borderBottomColor", resolve_color(c))],
        StyleProperty::BorderLeftColor(c) => vec![("borderLeftColor", resolve_color(c))],
        // React Native has no axis shorthand, so the two sides are written
        // out. It does have the inline-logical pair (`borderStartColor` /
        // `borderEndColor`), which is what keeps `border-s-*` correct under
        // RTL rather than being flattened to left/right.
        StyleProperty::BorderInlineColor(c) => vec![
            ("borderStartColor", resolve_color(c)),
            ("borderEndColor", resolve_color(c)),
        ],
        StyleProperty::BorderBlockColor(c) => vec![
            ("borderTopColor", resolve_color(c)),
            ("borderBottomColor", resolve_color(c)),
        ],
        StyleProperty::BorderInlineStartColor(c) => vec![("borderStartColor", resolve_color(c))],
        StyleProperty::BorderInlineEndColor(c) => vec![("borderEndColor", resolve_color(c))],
        // The block axis only diverges from top/bottom under a vertical
        // `writing-mode`, which React Native has no concept of -- the same
        // horizontal-only assumption `py-*` already lowers under.
        StyleProperty::BorderBlockStartColor(c) => vec![("borderTopColor", resolve_color(c))],
        StyleProperty::BorderBlockEndColor(c) => vec![("borderBottomColor", resolve_color(c))],
        // Unlike Web, RN defaults borderStyle to 'solid' and borderColor to
        // black, so a width alone already renders -- the opposite gotcha
        // from CSS's "invisible without border-style".
        StyleProperty::BorderTopWidth(l) => vec![("borderTopWidth", number(*l))],
        StyleProperty::BorderRightWidth(l) => vec![("borderRightWidth", number(*l))],
        StyleProperty::BorderBottomWidth(l) => vec![("borderBottomWidth", number(*l))],
        StyleProperty::BorderLeftWidth(l) => vec![("borderLeftWidth", number(*l))],
        // RN has no per-side border style -- one `borderStyle` covers all
        // four. Collapsing is safe here in a way it wouldn't be on Web:
        // RN defaults every border width to 0, so a style on a side with
        // no width renders nothing (whereas CSS would fall back to
        // `medium` and draw it).
        StyleProperty::BorderTopStyle(s)
        | StyleProperty::BorderRightStyle(s)
        | StyleProperty::BorderBottomStyle(s)
        | StyleProperty::BorderLeftStyle(s) => vec![("borderStyle", border_style_literal(s))],
        StyleProperty::BorderRadius(r) => vec![(
            "borderRadius",
            match r {
                Radius::Length(l) => number(*l),
                // RN has no infinity. Any radius past half the box's
                // shorter side already renders as a pill, so a large
                // finite value is the standard way to express this -- the
                // approximation is forced here, unlike on Web.
                Radius::Full => "9999".to_string(),
            },
        )],
        StyleProperty::BorderTopLeftRadius(r) => vec![("borderTopLeftRadius", radius_number(r))],
        StyleProperty::BorderTopRightRadius(r) => vec![("borderTopRightRadius", radius_number(r))],
        StyleProperty::BorderBottomRightRadius(r) => {
            vec![("borderBottomRightRadius", radius_number(r))]
        }
        StyleProperty::BorderBottomLeftRadius(r) => {
            vec![("borderBottomLeftRadius", radius_number(r))]
        }
        // React Native has the logical corner names too, so `rounded-s-*`
        // stays correct under RTL rather than being flattened to left/right.
        StyleProperty::BorderStartStartRadius(r) => {
            vec![("borderStartStartRadius", radius_number(r))]
        }
        StyleProperty::BorderStartEndRadius(r) => vec![("borderStartEndRadius", radius_number(r))],
        StyleProperty::BorderEndStartRadius(r) => vec![("borderEndStartRadius", radius_number(r))],
        StyleProperty::BorderEndEndRadius(r) => vec![("borderEndEndRadius", radius_number(r))],
        StyleProperty::FontSize(l) => vec![("fontSize", number(*l))],
        // RN's `fontWeight` type is a *string* ('100'..'900'/'normal'/
        // 'bold'), not a number -- unlike CSS's numeric font-weight.
        StyleProperty::FontWeight(w) => vec![("fontWeight", format!("'{}'", w.0))],
        StyleProperty::LineHeight(lh) => match lh {
            LineHeight::Length(l) => vec![("lineHeight", number(*l))],
            // Refused upstream (see `unsupported_on_native`); emitting
            // nothing keeps the object valid if that error is ignored.
            LineHeight::Ratio(_) => Vec::new(),
        },
        StyleProperty::Overflow(o) => vec![(
            "overflow",
            match o {
                Overflow::Visible => "'visible'",
                Overflow::Hidden => "'hidden'",
                Overflow::Scroll => "'scroll'",
            }
            .to_string(),
        )],
        // RN Text wraps by default, so `normal` is genuinely a no-op there.
        // `nowrap` is refused upstream -- suppressing wrapping needs the
        // `numberOfLines` prop, not a style.
        StyleProperty::WhiteSpace(_) => Vec::new(),
        // All refused upstream by `unsupported_on_native`.
        StyleProperty::LetterSpacing(_)
        | StyleProperty::TextOverflow(_)
        | StyleProperty::GridTemplateColumns(_)
        | StyleProperty::TransitionProperty(_)
        | StyleProperty::TransitionDuration(_)
        | StyleProperty::TransitionTimingFunction(_)
        | StyleProperty::Animation(_)
        | StyleProperty::SpaceX(_)
        | StyleProperty::SpaceY(_) => Vec::new(),
        StyleProperty::TextAlign(align) => vec![(
            "textAlign",
            match align {
                TextAlign::Left => "'left'",
                TextAlign::Center => "'center'",
                TextAlign::Right => "'right'",
            }
            .to_string(),
        )],
        // Composed into a single `transform` by the caller, since RN has no
        // standalone rotate/scale/translate -- see `transform_entry`.
        StyleProperty::Rotate(_)
        | StyleProperty::Scale(_)
        | StyleProperty::TranslateX(_)
        | StyleProperty::TranslateY(_) => Vec::new(),
        // RN accepts a string for both, so the CSS text carries over as-is.
        // Composed with any ring layers by `box_shadow_entry`, not emitted
        // here -- `style_pairs` filters these out before this runs.
        StyleProperty::BoxShadow(_)
        | StyleProperty::RingWidth(_)
        | StyleProperty::RingColor(_)
        | StyleProperty::InsetRingWidth(_)
        | StyleProperty::InsetRingColor(_) => vec![],
        StyleProperty::Filter(f) => vec![("filter", format!("'{f}'"))],
        StyleProperty::TextTransform(t) => vec![(
            "textTransform",
            match t {
                TextTransform::Uppercase => "'uppercase'",
                TextTransform::Lowercase => "'lowercase'",
                TextTransform::Capitalize => "'capitalize'",
                TextTransform::None => "'none'",
            }
            .to_string(),
        )],
        StyleProperty::TextColor(c) => vec![("color", resolve_color(c))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_have_no_unit_suffix() {
        assert_eq!(property_and_value(&StyleProperty::PaddingTop(Length::Px(24.0))), vec![("paddingTop", "24".to_string())]);
    }

    #[test]
    fn flex_grow_is_a_bare_number() {
        assert_eq!(
            property_and_value(&StyleProperty::Flex(FlexShorthand::Grow(1.0))),
            vec![("flex", "1".to_string())]
        );
    }

    #[test]
    fn flex_auto_expands_to_two_keys() {
        assert_eq!(
            property_and_value(&StyleProperty::Flex(FlexShorthand::Auto)),
            vec![("flexGrow", "1".to_string()), ("flexShrink", "1".to_string())]
        );
    }

    #[test]
    fn font_weight_is_a_string() {
        assert_eq!(
            property_and_value(&StyleProperty::FontWeight(dowel_ir::FontWeight(700))),
            vec![("fontWeight", "'700'".to_string())]
        );
    }

    #[test]
    fn known_color_token_resolves_to_real_hex() {
        assert_eq!(
            property_and_value(&StyleProperty::BackgroundColor(Color::Token("blue-500".to_string()))),
            vec![("backgroundColor", "'#2b7fff'".to_string())]
        );
    }

    #[test]
    fn unknown_color_token_falls_back_to_a_marker_string() {
        assert_eq!(
            property_and_value(&StyleProperty::TextColor(Color::Token("brand-primary".to_string()))),
            vec![("color", "'dowel-unresolved:brand-primary'".to_string())]
        );
    }
}
