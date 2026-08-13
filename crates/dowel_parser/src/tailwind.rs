//! Maps a single Tailwind utility class token to a `dowel_ir::StyleProperty`.
//!
//! Phase 0 scope only (proposal §13): flex layout, spacing, color,
//! typography. Unrecognized tokens return `None` rather than erroring --
//! callers decide what to do with an unmapped utility (Phase 0: drop it).

use dowel_ir::{
    Align, BorderStyle, Breakpoint, Color, Condition, Dimension, FlexDirection, FlexShorthand,
    FontWeight, Justify, Length, Position, StyleProperty, TextAlign,
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
        "w-auto" => return Some(StyleProperty::Width(Dimension::Auto)),
        "h-auto" => return Some(StyleProperty::Height(Dimension::Auto)),
        "text-left" => return Some(StyleProperty::TextAlign(TextAlign::Left)),
        "text-center" => return Some(StyleProperty::TextAlign(TextAlign::Center)),
        "text-right" => return Some(StyleProperty::TextAlign(TextAlign::Right)),
        "relative" => return Some(StyleProperty::Position(Position::Relative)),
        "absolute" => return Some(StyleProperty::Position(Position::Absolute)),
        "border-solid" => return Some(StyleProperty::BorderStyle(BorderStyle::Solid)),
        "border-dashed" => return Some(StyleProperty::BorderStyle(BorderStyle::Dashed)),
        "border-dotted" => return Some(StyleProperty::BorderStyle(BorderStyle::Dotted)),
        "border-none" => return Some(StyleProperty::BorderStyle(BorderStyle::None)),
        _ => {}
    }

    if let Some(radius) = parse_border_radius(token) {
        return Some(StyleProperty::BorderRadius(radius));
    }
    // `leading-<n>` only: Tailwind's *named* leading scale (`leading-tight`
    // = 1.25 etc.) is a unitless ratio of the element's own font size,
    // which `Length::Px` can't represent and which can't be resolved
    // statically -- so those fall through as unrecognized rather than being
    // converted to a wrong pixel value.
    if let Some(rest) = token.strip_prefix("leading-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::LineHeight(v));
        }
    }
    if let Some(rest) = token.strip_prefix("top-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::InsetTop(v));
        }
    }
    if let Some(rest) = token.strip_prefix("right-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::InsetRight(v));
        }
    }
    if let Some(rest) = token.strip_prefix("bottom-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::InsetBottom(v));
        }
    }
    if let Some(rest) = token.strip_prefix("left-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::InsetLeft(v));
        }
    }
    if let Some(rest) = token.strip_prefix("w-") {
        if let Some(d) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::Width(d));
        }
    }
    if let Some(rest) = token.strip_prefix("h-") {
        if let Some(d) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::Height(d));
        }
    }

    if let Some(weight) = parse_font_weight(token) {
        return Some(StyleProperty::FontWeight(weight));
    }
    // `text-<size>` sets font-size *and* line-height, so it can't fit this
    // one-property shape -- `expand_base_utility` handles it before ever
    // reaching here. It still has to be excluded explicitly, though,
    // because the `text-<color>` fallthrough below would otherwise swallow
    // `text-xl` as the color token "xl".
    if parse_font_size(token).is_some() {
        return None;
    }
    if let Some(prop) = parse_spacing_utility(token) {
        return Some(prop);
    }
    if let Some(rest) = token.strip_prefix("opacity-") {
        // Tailwind's opacity scale is 0-100 (in practice steps of 5),
        // meaning percent -- StyleProperty::Opacity wants the 0.0-1.0
        // fraction CSS/RN both expect.
        return rest.parse::<f32>().ok().map(|pct| StyleProperty::Opacity(pct / 100.0));
    }
    if let Some(color) = token.strip_prefix("bg-") {
        return Some(StyleProperty::BackgroundColor(Color::Token(color.to_string())));
    }
    if let Some(color) = token.strip_prefix("text-") {
        // Only reached if `parse_font_size`/`text-{left,center,right}` above
        // didn't match, so whatever remains is a color token (e.g. `blue-500`).
        return Some(StyleProperty::TextColor(Color::Token(color.to_string())));
    }
    if let Some(color) = token.strip_prefix("border-") {
        // Only reached once the width/style forms above have declined it,
        // so a non-numeric, non-keyword suffix here is a color token.
        return Some(StyleProperty::BorderColor(Color::Token(color.to_string())));
    }

    None
}

/// Tailwind's `--radius-*` scale, in px (its own values are rem at the
/// default 16px root). Bare `rounded` is 0.25rem, which is *not* the same
/// as `rounded-sm` in v4 -- they happen to share a value here but are
/// separate scale entries.
fn parse_border_radius(token: &str) -> Option<Length> {
    let px = match token {
        "rounded" => 4.0,
        "rounded-none" => 0.0,
        "rounded-xs" => 2.0,
        "rounded-sm" => 4.0,
        "rounded-md" => 6.0,
        "rounded-lg" => 8.0,
        "rounded-xl" => 12.0,
        "rounded-2xl" => 16.0,
        "rounded-3xl" => 24.0,
        "rounded-4xl" => 32.0,
        // Tailwind emits `calc(infinity * 1px)`; a large finite value is
        // the conventional equivalent and is what RN needs anyway (it has
        // no infinity).
        "rounded-full" => 9999.0,
        _ => return None,
    };
    Some(Length::Px(px))
}

/// Width/height accept more than the spacing scale: `w-1/2` fractions and
/// `w-full`/`w-auto` keywords (the latter handled by the exact-match table).
fn parse_dimension_suffix(suffix: &str) -> Option<Dimension> {
    if let Some((num, denom)) = suffix.split_once('/') {
        let num: f32 = num.parse().ok()?;
        let denom: f32 = denom.parse().ok()?;
        if denom == 0.0 {
            return None;
        }
        return Some(Dimension::Percent(num / denom * 100.0));
    }
    parse_spacing_suffix(suffix).map(Dimension::Length)
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

/// `(font-size, line-height)` in px. Tailwind's `text-*` utilities set
/// **both** -- its theme pairs each size with a `--text-*--line-height`,
/// and the generated CSS emits a `line-height` declaration alongside the
/// `font-size` one. Emitting only the font-size (as this did originally)
/// silently drops half of what the utility means.
///
/// Unlike the standalone named `leading-*` scale (a bare ratio against an
/// unknown font size, so unresolvable -- see `parse_utility`), these
/// resolve fine: the ratio's font size is the one this very utility sets,
/// so e.g. `text-xl` is 1.25rem x calc(1.75/1.25) = 1.75rem = 28px.
fn parse_font_size(token: &str) -> Option<(Length, Length)> {
    let (size, line_height) = match token {
        "text-xs" => (12.0, 16.0),
        "text-sm" => (14.0, 20.0),
        "text-base" => (16.0, 24.0),
        "text-lg" => (18.0, 28.0),
        "text-xl" => (20.0, 28.0),
        "text-2xl" => (24.0, 32.0),
        "text-3xl" => (30.0, 36.0),
        "text-4xl" => (36.0, 40.0),
        // From `text-5xl` up Tailwind's line-height ratio is a flat 1.
        "text-5xl" => (48.0, 48.0),
        "text-6xl" => (60.0, 60.0),
        "text-7xl" => (72.0, 72.0),
        "text-8xl" => (96.0, 96.0),
        "text-9xl" => (128.0, 128.0),
        _ => return None,
    };
    Some((Length::Px(size), Length::Px(line_height)))
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

/// Strips a single recognized `variant:` prefix (e.g. `hover:bg-blue-500`
/// -> `(Condition::Hover, "bg-blue-500")`). Only one level -- stacked
/// variants (`dark:hover:...`) aren't in the Condition model at all yet, so
/// there's nothing to strip them into.
pub fn parse_variant_prefix(token: &str) -> (Condition, &str) {
    if let Some(rest) = token.strip_prefix("hover:") {
        return (Condition::Hover, rest);
    }
    if let Some(rest) = token.strip_prefix("focus:") {
        return (Condition::Focus, rest);
    }
    if let Some(rest) = token.strip_prefix("disabled:") {
        return (Condition::Disabled, rest);
    }
    if let Some(rest) = token.strip_prefix("pressed:") {
        return (Condition::Pressed, rest);
    }
    if let Some(rest) = token.strip_prefix("sm:") {
        return (Condition::Responsive(Breakpoint::Sm), rest);
    }
    if let Some(rest) = token.strip_prefix("md:") {
        return (Condition::Responsive(Breakpoint::Md), rest);
    }
    if let Some(rest) = token.strip_prefix("lg:") {
        return (Condition::Responsive(Breakpoint::Lg), rest);
    }
    if let Some(rest) = token.strip_prefix("xl:") {
        return (Condition::Responsive(Breakpoint::Xl), rest);
    }
    if let Some(rest) = token.strip_prefix("2xl:") {
        return (Condition::Responsive(Breakpoint::Xl2), rest);
    }
    (Condition::Always, token)
}

/// The real entry point used by the JSX walker: strips a variant prefix
/// (if any) and expands the remaining base utility, returning the
/// condition that prefix implies alongside the properties it maps to.
pub fn expand_utility(token: &str) -> (Condition, Vec<StyleProperty>) {
    let (condition, base) = parse_variant_prefix(token);
    (condition, expand_base_utility(base))
}

/// Multi-side utilities (`p-6`, `px-4`, `py-2`, `m-6`, `mx-4`, `my-2`,
/// `gap-x-2`, `gap-y-2`) expand to more than one longhand property, so they
/// can't fit through `parse_utility`'s one-token-to-one-property shape.
fn expand_base_utility(token: &str) -> Vec<StyleProperty> {
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
    if let Some(rest) = token.strip_prefix("inset-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![
                StyleProperty::InsetTop(v),
                StyleProperty::InsetRight(v),
                StyleProperty::InsetBottom(v),
                StyleProperty::InsetLeft(v),
            ];
        }
    }
    if let Some(rest) = token.strip_prefix("size-") {
        if let Some(d) = parse_dimension_suffix(rest) {
            return vec![StyleProperty::Width(d), StyleProperty::Height(d)];
        }
    }
    if let Some(props) = expand_border_width(token) {
        return props;
    }
    if let Some((size, line_height)) = parse_font_size(token) {
        // Order matters: the line-height goes second so that an explicit
        // `leading-*` written *after* this class overrides it under
        // last-wins flattening. Note this is order-sensitive where real
        // Tailwind isn't -- Tailwind routes `leading-*` through a
        // `--tw-leading` custom property that wins regardless of class
        // order. Writing `leading-6 text-xl` therefore differs: Tailwind
        // keeps leading-6, Dowel takes text-xl's 28px.
        return vec![StyleProperty::FontSize(size), StyleProperty::LineHeight(line_height)];
    }

    parse_utility(token).into_iter().collect()
}

/// `border`, `border-<n>`, `border-{t,r,b,l}`, `border-{t,r,b,l}-<n>`.
///
/// Every border-width utility also emits `BorderStyle::Solid`, mirroring
/// Tailwind (which pairs each width with a style declaration) -- without
/// it CSS's default `border-style: none` means the width renders nothing.
///
/// Ordered after the color check would be ambiguous (`border-2` vs
/// `border-red-500`), so width parsing is tried first and only falls
/// through to color when the suffix isn't numeric.
fn expand_border_width(token: &str) -> Option<Vec<StyleProperty>> {
    let rest = token.strip_prefix("border")?;
    let solid = StyleProperty::BorderStyle(BorderStyle::Solid);

    // Bare `border` == 1px on every side.
    if rest.is_empty() {
        let one = Length::Px(1.0);
        return Some(vec![
            StyleProperty::BorderTopWidth(one),
            StyleProperty::BorderRightWidth(one),
            StyleProperty::BorderBottomWidth(one),
            StyleProperty::BorderLeftWidth(one),
            solid,
        ]);
    }

    let rest = rest.strip_prefix('-')?;
    let (side, width) = match rest.split_once('-') {
        // e.g. `border-t-2`
        Some((side, width)) if matches!(side, "t" | "r" | "b" | "l") => {
            (Some(side), parse_border_width_px(width)?)
        }
        // e.g. `border-t` -- a side with no width means 1px.
        None if matches!(rest, "t" | "r" | "b" | "l") => (Some(rest), Length::Px(1.0)),
        // e.g. `border-2`. Anything non-numeric here (`border-red-500`)
        // isn't a width at all, so this bails out and lets the color path
        // in `parse_utility` handle it.
        _ => (None, parse_border_width_px(rest)?),
    };

    Some(match side {
        Some("t") => vec![StyleProperty::BorderTopWidth(width), solid],
        Some("r") => vec![StyleProperty::BorderRightWidth(width), solid],
        Some("b") => vec![StyleProperty::BorderBottomWidth(width), solid],
        Some("l") => vec![StyleProperty::BorderLeftWidth(width), solid],
        _ => vec![
            StyleProperty::BorderTopWidth(width),
            StyleProperty::BorderRightWidth(width),
            StyleProperty::BorderBottomWidth(width),
            StyleProperty::BorderLeftWidth(width),
            solid,
        ],
    })
}

/// Border widths are plain pixel counts, not multiples of the spacing
/// scale -- `border-2` is 2px, not 8px.
fn parse_border_width_px(suffix: &str) -> Option<Length> {
    suffix.parse::<f32>().ok().map(Length::Px)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_opacity_scale() {
        assert_eq!(expand_utility("opacity-50"), (Condition::Always, vec![StyleProperty::Opacity(0.5)]));
        assert_eq!(
            expand_utility("disabled:opacity-50"),
            (Condition::Disabled, vec![StyleProperty::Opacity(0.5)])
        );
    }

    #[test]
    fn expands_login_example_utilities() {
        assert_eq!(
            expand_utility("flex-1"),
            (Condition::Always, vec![StyleProperty::Flex(FlexShorthand::Grow(1.0))])
        );
        assert_eq!(
            expand_utility("p-6"),
            (
                Condition::Always,
                vec![
                    StyleProperty::PaddingTop(Length::Px(24.0)),
                    StyleProperty::PaddingRight(Length::Px(24.0)),
                    StyleProperty::PaddingBottom(Length::Px(24.0)),
                    StyleProperty::PaddingLeft(Length::Px(24.0)),
                ]
            )
        );
        assert_eq!(
            expand_utility("px-4"),
            (
                Condition::Always,
                vec![StyleProperty::PaddingLeft(Length::Px(16.0)), StyleProperty::PaddingRight(Length::Px(16.0))]
            )
        );
        assert_eq!(
            expand_utility("text-xl"),
            (
                Condition::Always,
                vec![StyleProperty::FontSize(Length::Px(20.0)), StyleProperty::LineHeight(Length::Px(28.0))]
            )
        );
        assert_eq!(
            expand_utility("font-bold"),
            (Condition::Always, vec![StyleProperty::FontWeight(FontWeight(700))])
        );
        assert_eq!(
            expand_utility("bg-blue-500"),
            (Condition::Always, vec![StyleProperty::BackgroundColor(Color::Token("blue-500".to_string()))])
        );
        assert_eq!(expand_utility("unknown-utility"), (Condition::Always, Vec::<StyleProperty>::new()));
    }

    #[test]
    fn recognizes_variant_prefixes() {
        assert_eq!(
            expand_utility("hover:bg-blue-500"),
            (Condition::Hover, vec![StyleProperty::BackgroundColor(Color::Token("blue-500".to_string()))])
        );
        assert_eq!(
            expand_utility("focus:font-bold"),
            (Condition::Focus, vec![StyleProperty::FontWeight(FontWeight(700))])
        );
        assert_eq!(
            expand_utility("disabled:text-xl"),
            (
                Condition::Disabled,
                vec![StyleProperty::FontSize(Length::Px(20.0)), StyleProperty::LineHeight(Length::Px(28.0))]
            )
        );
        assert_eq!(
            expand_utility("md:flex-row"),
            (
                Condition::Responsive(Breakpoint::Md),
                vec![StyleProperty::FlexDirection(FlexDirection::Row)]
            )
        );
    }

    #[test]
    fn parses_position_and_inset() {
        assert_eq!(
            expand_utility("absolute"),
            (Condition::Always, vec![StyleProperty::Position(Position::Absolute)])
        );
        assert_eq!(
            expand_utility("top-4"),
            (Condition::Always, vec![StyleProperty::InsetTop(Length::Px(16.0))])
        );
        assert_eq!(
            expand_utility("inset-0"),
            (
                Condition::Always,
                vec![
                    StyleProperty::InsetTop(Length::Px(0.0)),
                    StyleProperty::InsetRight(Length::Px(0.0)),
                    StyleProperty::InsetBottom(Length::Px(0.0)),
                    StyleProperty::InsetLeft(Length::Px(0.0)),
                ]
            )
        );
    }

    #[test]
    fn border_width_always_carries_a_style_so_it_actually_renders() {
        // CSS defaults border-style to none, so a width with no style
        // renders nothing -- Tailwind pairs them for the same reason.
        let (_, props) = expand_utility("border");
        assert!(props.contains(&StyleProperty::BorderStyle(BorderStyle::Solid)));
        assert!(props.contains(&StyleProperty::BorderTopWidth(Length::Px(1.0))));
        assert_eq!(props.len(), 5);

        let (_, props) = expand_utility("border-2");
        assert!(props.contains(&StyleProperty::BorderLeftWidth(Length::Px(2.0))));

        // Per-side, bare and with an explicit width.
        assert_eq!(
            expand_utility("border-t").1,
            vec![
                StyleProperty::BorderTopWidth(Length::Px(1.0)),
                StyleProperty::BorderStyle(BorderStyle::Solid)
            ]
        );
        assert_eq!(
            expand_utility("border-b-4").1,
            vec![
                StyleProperty::BorderBottomWidth(Length::Px(4.0)),
                StyleProperty::BorderStyle(BorderStyle::Solid)
            ]
        );
    }

    #[test]
    fn border_color_is_not_mistaken_for_a_width() {
        assert_eq!(
            expand_utility("border-red-500"),
            (Condition::Always, vec![StyleProperty::BorderColor(Color::Token("red-500".to_string()))])
        );
    }

    #[test]
    fn parses_radius_scale() {
        assert_eq!(
            expand_utility("rounded-lg"),
            (Condition::Always, vec![StyleProperty::BorderRadius(Length::Px(8.0))])
        );
        assert_eq!(
            expand_utility("rounded"),
            (Condition::Always, vec![StyleProperty::BorderRadius(Length::Px(4.0))])
        );
    }

    #[test]
    fn parses_sizing_including_fractions_and_size_shorthand() {
        assert_eq!(
            expand_utility("w-4"),
            (Condition::Always, vec![StyleProperty::Width(Dimension::Length(Length::Px(16.0)))])
        );
        assert_eq!(
            expand_utility("w-1/2"),
            (Condition::Always, vec![StyleProperty::Width(Dimension::Percent(50.0))])
        );
        assert_eq!(
            expand_utility("size-4"),
            (
                Condition::Always,
                vec![
                    StyleProperty::Width(Dimension::Length(Length::Px(16.0))),
                    StyleProperty::Height(Dimension::Length(Length::Px(16.0))),
                ]
            )
        );
    }

    #[test]
    fn text_size_sets_line_height_too() {
        // Regression: this used to emit font-size only, silently dropping
        // the line-height half of what Tailwind's text-* utilities mean.
        for (token, size, line_height) in
            [("text-xs", 12.0, 16.0), ("text-base", 16.0, 24.0), ("text-4xl", 36.0, 40.0)]
        {
            assert_eq!(
                expand_utility(token),
                (
                    Condition::Always,
                    vec![
                        StyleProperty::FontSize(Length::Px(size)),
                        StyleProperty::LineHeight(Length::Px(line_height)),
                    ]
                ),
                "{token}"
            );
        }
        // From text-5xl up the ratio is a flat 1, so the two match.
        assert_eq!(
            expand_utility("text-5xl").1,
            vec![
                StyleProperty::FontSize(Length::Px(48.0)),
                StyleProperty::LineHeight(Length::Px(48.0))
            ]
        );
    }

    #[test]
    fn text_size_still_does_not_swallow_color_tokens() {
        // `text-<size>` is handled before the `text-<color>` fallthrough;
        // this guards the boundary between them in both directions.
        assert_eq!(
            expand_utility("text-red-500"),
            (Condition::Always, vec![StyleProperty::TextColor(Color::Token("red-500".to_string()))])
        );
        assert_eq!(
            expand_utility("text-center"),
            (Condition::Always, vec![StyleProperty::TextAlign(TextAlign::Center)])
        );
    }

    #[test]
    fn explicit_leading_after_a_text_size_wins() {
        // Dowel resolves this by source order (last wins), so `leading-*`
        // must be written after `text-*` to take effect. Real Tailwind is
        // order-independent here (it routes leading through a --tw-leading
        // custom property) -- a known, documented divergence.
        let (_, text_props) = expand_utility("text-xl");
        let (_, leading_props) = expand_utility("leading-6");
        let combined: Vec<_> = text_props.into_iter().chain(leading_props).collect();
        let deduped = dowel_ir::dedupe_last_wins(combined);
        assert!(deduped.contains(&StyleProperty::LineHeight(Length::Px(24.0))));
        assert!(!deduped.contains(&StyleProperty::LineHeight(Length::Px(28.0))));
    }

    #[test]
    fn named_leading_is_not_faked_as_pixels() {
        // Tailwind's named leading scale is a unitless ratio of the
        // element's own font size -- not statically resolvable to px, so it
        // stays unrecognized rather than being converted to a wrong value.
        assert_eq!(expand_utility("leading-tight"), (Condition::Always, vec![]));
        // The numeric scale *is* spacing-based and does resolve.
        assert_eq!(
            expand_utility("leading-6"),
            (Condition::Always, vec![StyleProperty::LineHeight(Length::Px(24.0))])
        );
    }

    #[test]
    fn pressed_variant_is_recognized() {
        assert_eq!(
            expand_utility("pressed:opacity-50"),
            (Condition::Pressed, vec![StyleProperty::Opacity(0.5)])
        );
    }
}
