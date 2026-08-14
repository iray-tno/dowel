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
    Align, BorderStyle, Color, Dimension, FlexDirection, FlexShorthand, Justify, Length, Position,
    StyleProperty, TextAlign,
};

fn number(length: Length) -> String {
    let Length::Px(value) = length;
    format!("{value}")
}

fn dimension_value(dim: Dimension) -> String {
    match dim {
        Dimension::Length(length) => number(length),
        Dimension::Percent(pct) => format!("'{pct}%'"),
        Dimension::Auto => "'auto'".to_string(),
    }
}

/// Resolves against the default Tailwind palette where possible (see module
/// docs); otherwise falls back to a marker string deliberately not
/// real-color-shaped, so a missed resolution fails loudly instead of
/// rendering a plausible-but-wrong color.
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
        StyleProperty::JustifyContent(justify) => vec![(
            "justifyContent",
            match justify {
                Justify::Start => "'flex-start'",
                Justify::Center => "'center'",
                Justify::End => "'flex-end'",
                Justify::Between => "'space-between'",
                Justify::Around => "'space-around'",
                Justify::Evenly => "'space-evenly'",
            }
            .to_string(),
        )],
        StyleProperty::Gap(l) => vec![("gap", number(*l))],
        StyleProperty::RowGap(l) => vec![("rowGap", number(*l))],
        StyleProperty::ColumnGap(l) => vec![("columnGap", number(*l))],
        StyleProperty::MarginTop(l) => vec![("marginTop", number(*l))],
        StyleProperty::MarginRight(l) => vec![("marginRight", number(*l))],
        StyleProperty::MarginBottom(l) => vec![("marginBottom", number(*l))],
        StyleProperty::MarginLeft(l) => vec![("marginLeft", number(*l))],
        StyleProperty::PaddingTop(l) => vec![("paddingTop", number(*l))],
        StyleProperty::PaddingRight(l) => vec![("paddingRight", number(*l))],
        StyleProperty::PaddingBottom(l) => vec![("paddingBottom", number(*l))],
        StyleProperty::PaddingLeft(l) => vec![("paddingLeft", number(*l))],
        // RN's own direction-relative props; they resolve against
        // `I18nManager.isRTL` at runtime, same role as CSS's inline-start/end.
        StyleProperty::MarginInlineStart(l) => vec![("marginStart", number(*l))],
        StyleProperty::MarginInlineEnd(l) => vec![("marginEnd", number(*l))],
        StyleProperty::PaddingInlineStart(l) => vec![("paddingStart", number(*l))],
        StyleProperty::PaddingInlineEnd(l) => vec![("paddingEnd", number(*l))],
        StyleProperty::Width(d) => vec![("width", dimension_value(*d))],
        StyleProperty::Height(d) => vec![("height", dimension_value(*d))],
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
        StyleProperty::BackgroundColor(c) => vec![("backgroundColor", resolve_color(c))],
        StyleProperty::Opacity(o) => vec![("opacity", format!("{o}"))],
        StyleProperty::BorderColor(c) => vec![("borderColor", resolve_color(c))],
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
        StyleProperty::BorderRadius(l) => vec![("borderRadius", number(*l))],
        StyleProperty::FontSize(l) => vec![("fontSize", number(*l))],
        // RN's `fontWeight` type is a *string* ('100'..'900'/'normal'/
        // 'bold'), not a number -- unlike CSS's numeric font-weight.
        StyleProperty::FontWeight(w) => vec![("fontWeight", format!("'{}'", w.0))],
        StyleProperty::LineHeight(l) => vec![("lineHeight", number(*l))],
        StyleProperty::TextAlign(align) => vec![(
            "textAlign",
            match align {
                TextAlign::Left => "'left'",
                TextAlign::Center => "'center'",
                TextAlign::Right => "'right'",
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
