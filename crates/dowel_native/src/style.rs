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
//! - `Color` stays an unresolved Tailwind token here too (proposal §16),
//!   but RN has nothing like a CSS custom property to defer resolution to
//!   -- the placeholder is a plainly-not-a-real-color marker string instead
//!   of syntax that looks valid but silently does nothing.

use dowel_ir::{
    Align, Color, Dimension, FlexDirection, FlexShorthand, Justify, Length, Position,
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

/// Not a real color -- an unresolved Tailwind token marker (see module
/// docs). Deliberately doesn't look like it could be a valid RN color
/// value, so a missed resolution pass fails loudly instead of rendering
/// with a plausible-but-wrong color.
fn color_placeholder(color: &Color) -> String {
    let Color::Token(token) = color;
    format!("'dowel-unresolved:{token}'")
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
        StyleProperty::BackgroundColor(c) => vec![("backgroundColor", color_placeholder(c))],
        StyleProperty::Opacity(o) => vec![("opacity", format!("{o}"))],
        StyleProperty::BorderColor(c) => vec![("borderColor", color_placeholder(c))],
        // Unlike Web, RN shows a (black, by default) border once
        // `borderWidth` is set even without an explicit `borderColor` --
        // the opposite gotcha from CSS's "invisible without borderStyle."
        StyleProperty::BorderWidth(l) => vec![("borderWidth", number(*l))],
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
        StyleProperty::TextColor(c) => vec![("color", color_placeholder(c))],
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
}
