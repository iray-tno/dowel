//! Maps a single Tailwind utility class token to a `dowel_ir::StyleProperty`.
//!
//! Phase 0 scope only (proposal §13): flex layout, spacing, color,
//! typography. Unrecognized tokens return `None` rather than erroring --
//! callers decide what to do with an unmapped utility (Phase 0: drop it).

use dowel_ir::{
    Align, Color, Dimension, FlexDirection, FlexShorthand, FontWeight, Justify, Length,
    StyleProperty, TextAlign,
};

/// Tailwind's default spacing scale: `spacing(n) = n * 0.25rem`, and the
/// default root font size is 16px, so each spacing step is 4px.
fn spacing_to_px(n: f32) -> Length {
    Length::Px(n * 4.0)
}

fn parse_spacing_suffix(suffix: &str) -> Option<Length> {
    if suffix == "px" {
        return Some(Length::Px(1.0));
    }
    suffix.parse::<f32>().ok().map(spacing_to_px)
}

pub fn parse_utility(token: &str) -> Option<StyleProperty> {
    match token {
        "flex-1" => return Some(StyleProperty::Flex(FlexShorthand::Grow(1.0))),
        "flex-auto" => return Some(StyleProperty::Flex(FlexShorthand::Auto)),
        "flex-initial" => return Some(StyleProperty::Flex(FlexShorthand::Initial)),
        "flex-none" => return Some(StyleProperty::Flex(FlexShorthand::None)),
        "flex-row" => return Some(StyleProperty::FlexDirection(FlexDirection::Row)),
        "flex-col" => return Some(StyleProperty::FlexDirection(FlexDirection::Column)),
        "flex-row-reverse" => {
            return Some(StyleProperty::FlexDirection(FlexDirection::RowReverse));
        }
        "flex-col-reverse" => {
            return Some(StyleProperty::FlexDirection(FlexDirection::ColumnReverse));
        }
        "items-start" => return Some(StyleProperty::AlignItems(Align::Start)),
        "items-center" => return Some(StyleProperty::AlignItems(Align::Center)),
        "items-end" => return Some(StyleProperty::AlignItems(Align::End)),
        "items-stretch" => return Some(StyleProperty::AlignItems(Align::Stretch)),
        "items-baseline" => return Some(StyleProperty::AlignItems(Align::Baseline)),
        "justify-start" => return Some(StyleProperty::JustifyContent(Justify::Start)),
        "justify-center" => return Some(StyleProperty::JustifyContent(Justify::Center)),
        "justify-end" => return Some(StyleProperty::JustifyContent(Justify::End)),
        "justify-between" => return Some(StyleProperty::JustifyContent(Justify::Between)),
        "justify-around" => return Some(StyleProperty::JustifyContent(Justify::Around)),
        "justify-evenly" => return Some(StyleProperty::JustifyContent(Justify::Evenly)),
        "w-full" => return Some(StyleProperty::Width(Dimension::Percent(100.0))),
        "h-full" => return Some(StyleProperty::Height(Dimension::Percent(100.0))),
        "text-left" => return Some(StyleProperty::TextAlign(TextAlign::Left)),
        "text-center" => return Some(StyleProperty::TextAlign(TextAlign::Center)),
        "text-right" => return Some(StyleProperty::TextAlign(TextAlign::Right)),
        _ => {}
    }

    if let Some(weight) = parse_font_weight(token) {
        return Some(StyleProperty::FontWeight(weight));
    }
    if let Some(size) = parse_font_size(token) {
        return Some(StyleProperty::FontSize(size));
    }
    if let Some(prop) = parse_spacing_utility(token) {
        return Some(prop);
    }
    if let Some(color) = token.strip_prefix("bg-") {
        return Some(StyleProperty::BackgroundColor(Color::Token(color.to_string())));
    }
    if let Some(color) = token.strip_prefix("text-") {
        // Only reached if `parse_font_size`/`text-{left,center,right}` above
        // didn't match, so whatever remains is a color token (e.g. `blue-500`).
        return Some(StyleProperty::TextColor(Color::Token(color.to_string())));
    }

    None
}

fn parse_font_weight(token: &str) -> Option<FontWeight> {
    let value = match token {
        "font-thin" => 100,
        "font-extralight" => 200,
        "font-light" => 300,
        "font-normal" => 400,
        "font-medium" => 500,
        "font-semibold" => 600,
        "font-bold" => 700,
        "font-extrabold" => 800,
        "font-black" => 900,
        _ => return None,
    };
    Some(FontWeight(value))
}

fn parse_font_size(token: &str) -> Option<Length> {
    let px = match token {
        "text-xs" => 12.0,
        "text-sm" => 14.0,
        "text-base" => 16.0,
        "text-lg" => 18.0,
        "text-xl" => 20.0,
        "text-2xl" => 24.0,
        "text-3xl" => 30.0,
        "text-4xl" => 36.0,
        _ => return None,
    };
    Some(Length::Px(px))
}

/// Handles `{p,px,py,pt,pr,pb,pl,m,mx,my,mt,mr,mb,ml,gap,gap-x,gap-y}-{n}`.
/// Multi-side prefixes (`p`, `px`, `py`, `m`, `mx`, `my`) expand to more than
/// one longhand `StyleProperty` -- but `parse_utility` returns a single
/// property, so callers that need the multi-side expansion should use
/// `parse_spacing_utility_all` instead. `p-6`/`px-4` etc. therefore aren't
/// handled here; see `parse_spacing_utility_all`.
fn parse_spacing_utility(token: &str) -> Option<StyleProperty> {
    let (prefix, rest) = token.split_once('-')?;
    let value = parse_spacing_suffix(rest)?;
    match prefix {
        "gap" => Some(StyleProperty::Gap(value)),
        "pt" => Some(StyleProperty::PaddingTop(value)),
        "pr" => Some(StyleProperty::PaddingRight(value)),
        "pb" => Some(StyleProperty::PaddingBottom(value)),
        "pl" => Some(StyleProperty::PaddingLeft(value)),
        "mt" => Some(StyleProperty::MarginTop(value)),
        "mr" => Some(StyleProperty::MarginRight(value)),
        "mb" => Some(StyleProperty::MarginBottom(value)),
        "ml" => Some(StyleProperty::MarginLeft(value)),
        _ => None,
    }
}

/// Multi-side utilities (`p-6`, `px-4`, `py-2`, `m-6`, `mx-4`, `my-2`,
/// `gap-x-2`, `gap-y-2`) expand to more than one longhand property, so they
/// can't fit through `parse_utility`'s one-token-to-one-property shape.
/// Called first by `expand_utility` (the real entry point used by the JSX
/// walker); falls through to `parse_utility` for everything else.
pub fn expand_utility(token: &str) -> Vec<StyleProperty> {
    if let Some(rest) = token.strip_prefix("px-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![StyleProperty::PaddingLeft(v), StyleProperty::PaddingRight(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("py-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![StyleProperty::PaddingTop(v), StyleProperty::PaddingBottom(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("p-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![
                StyleProperty::PaddingTop(v),
                StyleProperty::PaddingRight(v),
                StyleProperty::PaddingBottom(v),
                StyleProperty::PaddingLeft(v),
            ];
        }
    }
    if let Some(rest) = token.strip_prefix("mx-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![StyleProperty::MarginLeft(v), StyleProperty::MarginRight(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("my-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![StyleProperty::MarginTop(v), StyleProperty::MarginBottom(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("m-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![
                StyleProperty::MarginTop(v),
                StyleProperty::MarginRight(v),
                StyleProperty::MarginBottom(v),
                StyleProperty::MarginLeft(v),
            ];
        }
    }
    if let Some(rest) = token.strip_prefix("gap-x-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![StyleProperty::ColumnGap(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("gap-y-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![StyleProperty::RowGap(v)];
        }
    }

    parse_utility(token).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_login_example_utilities() {
        assert_eq!(
            expand_utility("flex-1"),
            vec![StyleProperty::Flex(FlexShorthand::Grow(1.0))]
        );
        assert_eq!(
            expand_utility("p-6"),
            vec![
                StyleProperty::PaddingTop(Length::Px(24.0)),
                StyleProperty::PaddingRight(Length::Px(24.0)),
                StyleProperty::PaddingBottom(Length::Px(24.0)),
                StyleProperty::PaddingLeft(Length::Px(24.0)),
            ]
        );
        assert_eq!(
            expand_utility("px-4"),
            vec![StyleProperty::PaddingLeft(Length::Px(16.0)), StyleProperty::PaddingRight(Length::Px(16.0))]
        );
        assert_eq!(expand_utility("text-xl"), vec![StyleProperty::FontSize(Length::Px(20.0))]);
        assert_eq!(expand_utility("font-bold"), vec![StyleProperty::FontWeight(FontWeight(700))]);
        assert_eq!(
            expand_utility("bg-blue-500"),
            vec![StyleProperty::BackgroundColor(Color::Token("blue-500".to_string()))]
        );
        assert_eq!(expand_utility("unknown-utility"), Vec::<StyleProperty>::new());
    }
}
