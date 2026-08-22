//! Maps a single Tailwind utility class token to a `hozo_ir::StyleProperty`.
//!
//! Phase 0 scope only (proposal §13): flex layout, spacing, color,
//! typography. Unrecognized tokens return `None` rather than erroring --
//! callers decide what to do with an unmapped utility (Phase 0: drop it).

use hozo_ir::{
    Align, AlignSelf, Angle, Animation, BorderStyle, Breakpoint, Clamp, Color, Condition, Dimension,
    ColumnCount, DecorationStyle, Display, Edge, Em, GradientKind, GradientStop, GridLine, GridSpan,
    GridTracks, LetterSpacing,
    MaskSlot, MaskStop,
    FilterFunction, FlexDirection, FlexShorthand, FontWeight, Justify, Length, LineHeight, Overflow,
    Position, Radius, Scale, StyleProperty, TextAlign, TextOverflow, TextTransform, WhiteSpace,
};

/// The property lists Tailwind's `transition`/`transition-colors` expand
/// to, copied verbatim so the emitted CSS matches. Long, but that's what
/// the utility means -- shortening it would change behaviour.
const DEFAULT_TRANSITION_PROPERTIES: &str = "color, background-color, border-color, outline-color, \
    text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to, \
    opacity, box-shadow, transform, translate, scale, rotate, filter, -webkit-backdrop-filter, \
    backdrop-filter, display, content-visibility, overlay, pointer-events";

const COLOR_TRANSITION_PROPERTIES: &str = "color, background-color, border-color, outline-color, \
    text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to";

/// Tailwind's `--default-transition-*`, applied by every `transition-*`
/// utility unless an explicit `duration-*`/`ease-*` overrides them.
const DEFAULT_TRANSITION_TIMING: &str = "cubic-bezier(0.4, 0, 0.2, 1)";
const DEFAULT_TRANSITION_DURATION_MS: u32 = 150;

fn parse_transition_properties(token: &str) -> Option<&'static str> {
    Some(match token {
        "transition" => DEFAULT_TRANSITION_PROPERTIES,
        "transition-colors" => COLOR_TRANSITION_PROPERTIES,
        "transition-opacity" => "opacity",
        "transition-transform" => "transform, translate, scale, rotate",
        "transition-shadow" => "box-shadow",
        "transition-all" => "all",
        "transition-none" => "none",
        _ => return None,
    })
}

/// A spacing suffix as the step count it is, not the pixels it works out
/// to.
///
/// `p-4` is four steps; how wide a step is belongs to the project
/// (`--spacing`, 0.25rem by default) and is applied at lowering, where the
/// theme is. Multiplying here baked Tailwind's default in, so a project
/// that changed the scale got the right number of steps at the wrong
/// size -- and silently, because the output was an ordinary padding.
///
/// `p-px` is the exception and stays absolute: Tailwind means one physical
/// pixel by it, which is what a hairline border wants and is not a step of
/// anything.
fn parse_spacing_suffix(suffix: &str) -> Option<Length> {
    if suffix == "px" {
        return Some(Length::Px(1.0));
    }
    suffix.parse::<f64>().ok().map(Length::Spacing)
}

/// Re-exported so callers can ask "was this class asking for something?"
/// without reaching past this module for the answer.
pub use crate::arbitrary::is_arbitrary;

pub fn parse_utility(token: &str) -> Option<StyleProperty> {
    match token {
        "flex-auto" => return Some(StyleProperty::Flex(FlexShorthand::Auto)),
        "shrink" => return Some(StyleProperty::FlexShrink(1.0)),
        "shrink-0" => return Some(StyleProperty::FlexShrink(0.0)),
        "grow" => return Some(StyleProperty::FlexGrow(1.0)),
        "grow-0" => return Some(StyleProperty::FlexGrow(0.0)),
        "z-auto" => return Some(StyleProperty::ZIndex(None)),
        "aspect-square" => return Some(StyleProperty::AspectRatio("1 / 1")),
        "aspect-video" => return Some(StyleProperty::AspectRatio("16 / 9")),
        "aspect-auto" => return Some(StyleProperty::AspectRatio("auto")),
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
        // Web-only: refused by the Native backend rather than dropped.
        "w-screen" => return Some(StyleProperty::Width(Dimension::ViewportWidth(100.0))),
        "h-screen" => return Some(StyleProperty::Height(Dimension::ViewportHeight(100.0))),
        "min-h-screen" => return Some(StyleProperty::MinHeight(Dimension::ViewportHeight(100.0))),
        "max-h-screen" => return Some(StyleProperty::MaxHeight(Dimension::ViewportHeight(100.0))),
        "text-left" => return Some(StyleProperty::TextAlign(TextAlign::Left)),
        "text-center" => return Some(StyleProperty::TextAlign(TextAlign::Center)),
        "text-right" => return Some(StyleProperty::TextAlign(TextAlign::Right)),
        "relative" => return Some(StyleProperty::Position(Position::Relative)),
        "absolute" => return Some(StyleProperty::Position(Position::Absolute)),
        "flex" => return Some(StyleProperty::Display(Display::Flex)),
        "hidden" => return Some(StyleProperty::Display(Display::None)),
        "contents" => return Some(StyleProperty::Display(Display::Contents)),
        // Accepted here and refused later by the Native backend, rather
        // than dropped at parse time: the Web backend can lower them fine,
        // and a build error naming the class beats silence.
        "block" => return Some(StyleProperty::Display(Display::Block)),
        "inline-flex" => return Some(StyleProperty::Display(Display::InlineFlex)),
        "grid" => return Some(StyleProperty::Display(Display::Grid)),
        "self-auto" => return Some(StyleProperty::AlignSelf(AlignSelf::Auto)),
        "self-start" => return Some(StyleProperty::AlignSelf(AlignSelf::Start)),
        "self-center" => return Some(StyleProperty::AlignSelf(AlignSelf::Center)),
        "self-end" => return Some(StyleProperty::AlignSelf(AlignSelf::End)),
        "self-stretch" => return Some(StyleProperty::AlignSelf(AlignSelf::Stretch)),
        "self-baseline" => return Some(StyleProperty::AlignSelf(AlignSelf::Baseline)),
        "content-start" => return Some(StyleProperty::AlignContent(Justify::Start)),
        "content-center" => return Some(StyleProperty::AlignContent(Justify::Center)),
        "content-end" => return Some(StyleProperty::AlignContent(Justify::End)),
        "content-between" => return Some(StyleProperty::AlignContent(Justify::Between)),
        "content-around" => return Some(StyleProperty::AlignContent(Justify::Around)),
        "content-evenly" => return Some(StyleProperty::AlignContent(Justify::Evenly)),
        "animate-spin" => return Some(StyleProperty::Animation(Animation::Spin)),
        "animate-ping" => return Some(StyleProperty::Animation(Animation::Ping)),
        "animate-pulse" => return Some(StyleProperty::Animation(Animation::Pulse)),
        "animate-bounce" => return Some(StyleProperty::Animation(Animation::Bounce)),
        "animate-none" => return Some(StyleProperty::Animation(Animation::None)),
        "overflow-hidden" => return Some(StyleProperty::Overflow(Overflow::Hidden)),
        "overflow-visible" => return Some(StyleProperty::Overflow(Overflow::Visible)),
        "overflow-scroll" => return Some(StyleProperty::Overflow(Overflow::Scroll)),
        "whitespace-nowrap" => return Some(StyleProperty::WhiteSpace(WhiteSpace::NoWrap)),
        "whitespace-normal" => return Some(StyleProperty::WhiteSpace(WhiteSpace::Normal)),
        "text-ellipsis" => return Some(StyleProperty::TextOverflow(TextOverflow::Ellipsis)),
        "text-clip" => return Some(StyleProperty::TextOverflow(TextOverflow::Clip)),
        // `transition-*` also carries the default timing/duration, so it's
        // a multi-property expansion handled in `expand_base_utility`.
        "ease-linear" => {
            return Some(StyleProperty::TransitionTimingFunction("linear".to_string()))
        }
        "ease-in" => {
            return Some(StyleProperty::TransitionTimingFunction(
                "cubic-bezier(0.4, 0, 1, 1)".to_string(),
            ))
        }
        "ease-out" => {
            return Some(StyleProperty::TransitionTimingFunction(
                "cubic-bezier(0, 0, 0.2, 1)".to_string(),
            ))
        }
        "ease-in-out" => {
            return Some(StyleProperty::TransitionTimingFunction(
                "cubic-bezier(0.4, 0, 0.2, 1)".to_string(),
            ))
        }
        "uppercase" => return Some(StyleProperty::TextTransform(TextTransform::Uppercase)),
        "lowercase" => return Some(StyleProperty::TextTransform(TextTransform::Lowercase)),
        "capitalize" => return Some(StyleProperty::TextTransform(TextTransform::Capitalize)),
        "normal-case" => return Some(StyleProperty::TextTransform(TextTransform::None)),
        // `border-{solid,dashed,...}` set all four sides, so they live in
        // `expand_base_utility` (multi-property) and never reach here.
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
        // Named scale first: those are unitless ratios, not lengths.
        if let Some(ratio) = parse_named_leading(rest) {
            return Some(StyleProperty::LineHeight(LineHeight::Ratio(ratio)));
        }
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::LineHeight(LineHeight::Length(v)));
        }
    }
    if let Some(rest) = token.strip_prefix("tracking-") {
        if let Some(em) = parse_tracking(rest) {
            return Some(StyleProperty::LetterSpacing(LetterSpacing::Em(Em(em))));
        }
    }
    if let Some(rest) = token.strip_prefix("duration-") {
        if let Ok(ms) = rest.parse::<u32>() {
            return Some(StyleProperty::TransitionDuration(ms));
        }
    }
    if let Some(prop) = parse_grid_placement(token) {
        return Some(prop);
    }
    if let Some(rest) = token.strip_prefix("space-x-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::SpaceX(Dimension::Length(v)));
        }
    }
    if let Some(rest) = token.strip_prefix("space-y-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::SpaceY(Dimension::Length(v)));
        }
    }
    if let Some(rest) = token.strip_prefix("top-") {
        if let Some(v) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::InsetTop(v));
        }
    }
    if let Some(rest) = token.strip_prefix("right-") {
        if let Some(v) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::InsetRight(v));
        }
    }
    if let Some(rest) = token.strip_prefix("bottom-") {
        if let Some(v) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::InsetBottom(v));
        }
    }
    if let Some(rest) = token.strip_prefix("start-") {
        if let Some(v) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::InsetInlineStart(v));
        }
    }
    if let Some(rest) = token.strip_prefix("end-") {
        if let Some(v) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::InsetInlineEnd(v));
        }
    }
    if let Some(rest) = token.strip_prefix("left-") {
        if let Some(v) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::InsetLeft(v));
        }
    }
    // Inline axis takes the container scale, block axis doesn't -- see
    // `parse_inline_size_suffix`.
    if let Some(rest) = token.strip_prefix("w-") {
        if let Some(d) = parse_inline_size_suffix(rest) {
            return Some(StyleProperty::Width(d));
        }
    }
    if let Some(rest) = token.strip_prefix("h-") {
        if let Some(d) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::Height(d));
        }
    }
    if let Some(rest) = token.strip_prefix("min-w-") {
        if let Some(d) = parse_extremum_suffix(rest) {
            return Some(StyleProperty::MinWidth(d));
        }
        if rest == "screen" {
            return Some(StyleProperty::MinWidth(Dimension::ViewportWidth(100.0)));
        }
        if let Some(d) = parse_inline_size_suffix(rest) {
            return Some(StyleProperty::MinWidth(d));
        }
    }
    if let Some(rest) = token.strip_prefix("min-h-") {
        if let Some(d) = parse_extremum_suffix(rest) {
            return Some(StyleProperty::MinHeight(d));
        }
        if let Some(d) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::MinHeight(d));
        }
    }
    if let Some(rest) = token.strip_prefix("max-w-") {
        if let Some(d) = parse_extremum_suffix(rest) {
            return Some(StyleProperty::MaxWidth(d));
        }
        if let Some(d) = parse_inline_size_suffix(rest) {
            return Some(StyleProperty::MaxWidth(d));
        }
    }
    if let Some(rest) = token.strip_prefix("max-h-") {
        if let Some(d) = parse_extremum_suffix(rest) {
            return Some(StyleProperty::MaxHeight(d));
        }
        if let Some(d) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::MaxHeight(d));
        }
    }
    if let Some(rest) = token.strip_prefix("z-") {
        if let Ok(z) = rest.parse::<i32>() {
            return Some(StyleProperty::ZIndex(Some(z)));
        }
    }
    if let Some(prop) = parse_order(token) {
        return Some(prop);
    }
    if let Some(rest) = token.strip_prefix("columns-") {
        if let Some(columns) = parse_columns_suffix(rest) {
            return Some(StyleProperty::Columns(columns));
        }
    }
    if let Some(keyword) = parse_cursor(token) {
        return Some(StyleProperty::Cursor(keyword.to_string()));
    }
    if let Some(rest) = token.strip_prefix("inset-shadow") {
        let shadow = match rest.trim_start_matches('-') {
            "2xs" => "inset 0 1px rgb(0 0 0 / 0.05)",
            "xs" => "inset 0 1px 1px rgb(0 0 0 / 0.05)",
            "sm" => "inset 0 2px 4px rgb(0 0 0 / 0.05)",
            // Tailwind clears the layer to a fully transparent inset
            // rather than dropping it, so a ring beside it still paints.
            "none" => "inset 0 0 #0000",
            // Anything else after the prefix is a colour for the layer,
            // which is a second utility rather than a second shadow --
            // `inset-shadow-sm inset-shadow-red-500` is one red shadow.
            suffix if !suffix.is_empty() => {
                return Some(StyleProperty::InsetShadowColor(Color::Token(suffix.to_string())))
            }
            _ => return None,
        };
        return Some(StyleProperty::InsetShadow(shadow.to_string()));
    }
    if let Some(shadow) = parse_shadow(token) {
        return Some(StyleProperty::BoxShadow(shadow.to_string()));
    }
    // Same split as the inset form: a suffix the size table declined is a
    // colour. After `parse_shadow`, so `shadow-sm` stays a size.
    if let Some(suffix) = token.strip_prefix("shadow-") {
        return Some(StyleProperty::ShadowColor(Color::Token(suffix.to_string())));
    }
    if let Some(prop) = parse_transform(token) {
        return Some(prop);
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
    if let Some(prop) = parse_single_margin(token) {
        return Some(prop);
    }
    if let Some(rest) = token.strip_prefix("opacity-") {
        // Tailwind's opacity scale is 0-100 (in practice steps of 5),
        // meaning percent -- StyleProperty::Opacity wants the 0.0-1.0
        // fraction CSS/RN both expect.
        return rest.parse::<f64>().ok().map(|pct| StyleProperty::Opacity(pct / 100.0));
    }
    if let Some(color) = token.strip_prefix("bg-") {
        if is_non_color_suffix(ColorFamily::Background, color) {
            return None;
        }
        return Some(StyleProperty::BackgroundColor(Color::Token(color.to_string())));
    }
    if let Some(color) = token.strip_prefix("text-") {
        // Only reached if `parse_font_size`/`text-{left,center,right}` above
        // didn't match, so whatever remains is a color token (e.g. `blue-500`).
        if is_non_color_suffix(ColorFamily::Text, color) {
            return None;
        }
        return Some(StyleProperty::TextColor(Color::Token(color.to_string())));
    }
    if let Some(color) = token.strip_prefix("border-") {
        // Only reached once the width/style forms above have declined it,
        // so a non-numeric, non-keyword suffix here is a color token.
        if let Some(prop) = parse_border_side_color(color) {
            return Some(prop);
        }
        if is_non_color_suffix(ColorFamily::Border, color) {
            return None;
        }
        return Some(StyleProperty::BorderColor(Color::Token(color.to_string())));
    }

    None
}

/// The one-property spacing/sizing families that take a `Dimension`.
///
/// A flat prefix table because that is all they are -- each is one CSS
/// property fed by the same value parser everything else uses.
fn expand_dimension_family(token: &str) -> Option<Vec<StyleProperty>> {
    /// Whether the family sits on the inline axis, and so also accepts the
    /// named container scale (`basis-md`). See `parse_inline_size_suffix`.
    const INLINE: bool = true;
    const BLOCK: bool = false;
    const FAMILIES: &[(&str, fn(Dimension) -> StyleProperty, bool)] = &[
        ("basis-", StyleProperty::FlexBasis, INLINE),
        ("block-", StyleProperty::BlockSize, BLOCK),
        ("inline-", StyleProperty::InlineSize, INLINE),
        ("max-block-", StyleProperty::MaxBlockSize, BLOCK),
        ("max-inline-", StyleProperty::MaxInlineSize, INLINE),
        ("min-block-", StyleProperty::MinBlockSize, BLOCK),
        ("min-inline-", StyleProperty::MinInlineSize, INLINE),
        ("indent-", StyleProperty::TextIndent, BLOCK),
        ("mbs-", StyleProperty::MarginBlockStart, BLOCK),
        ("mbe-", StyleProperty::MarginBlockEnd, BLOCK),
    ];
    // Longest prefix first: `max-block-` must beat `block-`.
    for (prefix, make, inline_axis) in
        FAMILIES.iter().filter(|(p, _, _)| token.starts_with(*p)).max_by_key(|(p, _, _)| p.len())
    {
        let suffix = &token[prefix.len()..];
        if prefix.starts_with("max-") || prefix.starts_with("min-") {
            if let Some(value) = parse_extremum_suffix(suffix) {
                return Some(vec![make(value)]);
            }
        }
        let value = if *inline_axis {
            parse_inline_size_suffix(suffix)
        } else {
            parse_dimension_suffix(suffix)
        };
        if let Some(value) = value {
            return Some(vec![make(value)]);
        }
    }
    // A plain pixel count, not the spacing scale: `underline-offset-4` is
    // 4px, where `p-4` is 16px. Tailwind uses the bare number here because
    // an underline offset is a typographic distance rather than a layout
    // step.
    if let Some(rest) = token.strip_prefix("underline-offset-") {
        if rest == "auto" {
            return Some(vec![StyleProperty::TextUnderlineOffset(Dimension::Auto)]);
        }
        if let Ok(px) = rest.parse::<f64>() {
            return Some(vec![StyleProperty::TextUnderlineOffset(Dimension::Length(
                Length::Px(px),
            ))]);
        }
    }
    if let Some(rest) = token.strip_prefix("pbs-") {
        return parse_spacing_suffix(rest).map(|l| vec![StyleProperty::PaddingBlockStart(l)]);
    }
    if let Some(rest) = token.strip_prefix("pbe-") {
        return parse_spacing_suffix(rest).map(|l| vec![StyleProperty::PaddingBlockEnd(l)]);
    }
    if let Some(rest) = token.strip_prefix("border-spacing-x-") {
        return parse_spacing_suffix(rest)
            .map(|l| vec![StyleProperty::BorderSpacingX(Dimension::Length(l))]);
    }
    if let Some(rest) = token.strip_prefix("border-spacing-y-") {
        return parse_spacing_suffix(rest)
            .map(|l| vec![StyleProperty::BorderSpacingY(Dimension::Length(l))]);
    }
    if let Some(rest) = token.strip_prefix("border-spacing-") {
        return parse_spacing_suffix(rest)
            .map(|l| {
                vec![
                    StyleProperty::BorderSpacingX(Dimension::Length(l)),
                    StyleProperty::BorderSpacingY(Dimension::Length(l)),
                ]
            });
    }
    // `*-screen` is a viewport size, which the block/inline families spell
    // on their own axis: `block-screen` is a height, `inline-screen` a
    // width. Handled before the table so it stays a `ViewportHeight`/
    // `ViewportWidth` the Native backend can answer, rather than text.
    for (prefix, make, vertical) in [
        ("max-block-", StyleProperty::MaxBlockSize as fn(Dimension) -> StyleProperty, true),
        ("min-block-", StyleProperty::MinBlockSize as fn(Dimension) -> StyleProperty, true),
        ("block-", StyleProperty::BlockSize as fn(Dimension) -> StyleProperty, true),
        ("max-inline-", StyleProperty::MaxInlineSize as fn(Dimension) -> StyleProperty, false),
        ("min-inline-", StyleProperty::MinInlineSize as fn(Dimension) -> StyleProperty, false),
        ("inline-", StyleProperty::InlineSize as fn(Dimension) -> StyleProperty, false),
    ] {
        if token == format!("{}screen", prefix) {
            return Some(vec![make(if vertical {
                Dimension::ViewportHeight(100.0)
            } else {
                Dimension::ViewportWidth(100.0)
            })]);
        }
    }
    // Bare `translate-*` sets both axes; `translate-z-*` the third.
    if let Some(rest) = token.strip_prefix("translate-z-") {
        return parse_spacing_suffix(rest).map(|l| vec![StyleProperty::TranslateZ(Dimension::Length(l))]);
    }
    if let Some(rest) = token.strip_prefix("translate-") {
        return parse_dimension_suffix(rest)
            .map(|d| vec![StyleProperty::TranslateX(d.clone()), StyleProperty::TranslateY(d)]);
    }
    parse_axis_transform(token)
}

/// The per-axis scales, the 3D rotations and the skews.
///
/// All of these are compositions in Tailwind -- several utilities writing
/// `--tw-*` registers that one declaration reads -- so Hozo resolves them
/// the same way it resolves rings, masks and translates: hold each axis as
/// its own property and join them at emit time. Bare `scale-*` sets all
/// three axes here rather than staying a fourth property, which is what
/// makes `scale-50 scale-x-75` resolve as Tailwind does: `dedupe_last_wins`
/// keys on the property, so the axes have to be separate properties to
/// override one another.
fn parse_axis_transform(token: &str) -> Option<Vec<StyleProperty>> {
    if token == "scale-3d" {
        return Some(vec![StyleProperty::Scale3d]);
    }
    let percent = |rest: &str| rest.parse::<f64>().ok().map(Scale::Percent);
    let degrees = |rest: &str| rest.parse::<f64>().ok().map(Angle::Deg);

    if let Some(rest) = token.strip_prefix("scale-x-") {
        return percent(rest).map(|p| vec![StyleProperty::ScaleX(p)]);
    }
    if let Some(rest) = token.strip_prefix("scale-y-") {
        return percent(rest).map(|p| vec![StyleProperty::ScaleY(p)]);
    }
    // Writing the z axis also switches the declaration to its three-value
    // form -- see `StyleProperty::Scale3d`.
    if let Some(rest) = token.strip_prefix("scale-z-") {
        return percent(rest).map(|p| vec![StyleProperty::ScaleZ(p), StyleProperty::Scale3d]);
    }
    if let Some(rest) = token.strip_prefix("scale-") {
        return percent(rest).map(|p| {
            vec![StyleProperty::ScaleX(p.clone()), StyleProperty::ScaleY(p.clone()), StyleProperty::ScaleZ(p)]
        });
    }
    if let Some(rest) = token.strip_prefix("rotate-x-") {
        return degrees(rest).map(|a| vec![StyleProperty::RotateX(a)]);
    }
    if let Some(rest) = token.strip_prefix("rotate-y-") {
        return degrees(rest).map(|a| vec![StyleProperty::RotateY(a)]);
    }
    if let Some(rest) = token.strip_prefix("rotate-z-") {
        return degrees(rest).map(|a| vec![StyleProperty::RotateZ(a)]);
    }
    if let Some(rest) = token.strip_prefix("skew-x-") {
        return degrees(rest).map(|a| vec![StyleProperty::SkewX(a)]);
    }
    if let Some(rest) = token.strip_prefix("skew-y-") {
        return degrees(rest).map(|a| vec![StyleProperty::SkewY(a)]);
    }
    // Bare `skew-*` is both axes, the same way bare `scale-*` is all three.
    if let Some(rest) = token.strip_prefix("skew-") {
        return degrees(rest).map(|a| vec![StyleProperty::SkewX(a.clone()), StyleProperty::SkewY(a)]);
    }
    None
}

fn expand_scrollbar(token: &str) -> Option<StyleProperty> {
    let rest = token.strip_prefix("scrollbar-")?;
    if let Some(colour) = rest.strip_prefix("thumb-") {
        return Some(StyleProperty::ScrollbarThumbColor(Color::Token(colour.to_string())));
    }
    if let Some(colour) = rest.strip_prefix("track-") {
        return Some(StyleProperty::ScrollbarTrackColor(Color::Token(colour.to_string())));
    }
    Some(match rest {
        "auto" => StyleProperty::ScrollbarWidth("auto"),
        "none" => StyleProperty::ScrollbarWidth("none"),
        "thin" => StyleProperty::ScrollbarWidth("thin"),
        "gutter-auto" => StyleProperty::ScrollbarGutter("auto"),
        "gutter-stable" => StyleProperty::ScrollbarGutter("stable"),
        "gutter-both" => StyleProperty::ScrollbarGutter("stable both-edges"),
        _ => return None,
    })
}

/// The gradient half of `mask-*`: stops, angles, and the radial shaping
/// utilities.
///
/// `mask-x-*`/`mask-y-*` are the only ones that produce two properties --
/// they set both edges of an axis, exactly as Tailwind does.
fn expand_mask_gradient(token: &str) -> Option<Vec<StyleProperty>> {
    let (negative, token) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    let rest = token.strip_prefix("mask-")?;

    // Radial shaping. These paint nothing on their own; they only change
    // how a radial gradient from some other utility is drawn.
    if let Some(position) = rest.strip_prefix("radial-at-") {
        return radial_position(position).map(|p| vec![StyleProperty::MaskRadialPosition(p)]);
    }
    match rest {
        "circle" => return Some(vec![StyleProperty::MaskRadialShape("circle")]),
        "ellipse" => return Some(vec![StyleProperty::MaskRadialShape("ellipse")]),
        "add" => return Some(vec![StyleProperty::MaskComposite("add")]),
        "subtract" => return Some(vec![StyleProperty::MaskComposite("subtract")]),
        "intersect" => return Some(vec![StyleProperty::MaskComposite("intersect")]),
        "exclude" => return Some(vec![StyleProperty::MaskComposite("exclude")]),
        _ => {}
    }
    if let Some(size) = rest.strip_prefix("radial-") {
        if matches!(
            size,
            "closest-side" | "closest-corner" | "farthest-side" | "farthest-corner"
        ) {
            return Some(vec![StyleProperty::MaskRadialSize(leak_size(size).to_string())]);
        }
    }

    let (axis, tail) = rest.split_once('-')?;
    let slots = mask_slots(axis)?;

    // `mask-linear-45` / `mask-conic-45`: the whole tail is an angle.
    if let Ok(degrees) = tail.parse::<f64>() {
        if matches!(axis, "linear" | "conic") {
            return Some(vec![StyleProperty::MaskSlotArgument(slots[0], Angle::Deg(signed(degrees, negative)))]);
        }
        return None;
    }

    let (stop, value) = match tail.split_once('-') {
        Some(("from", value)) => (MaskStop::From, value),
        Some(("to", value)) => (MaskStop::To, value),
        _ => return None,
    };

    // A stop is either a position on the spacing scale / a percentage, or
    // a colour. Positions are tried first because a bare number is never a
    // colour token.
    let property: Box<dyn Fn(MaskSlot) -> StyleProperty> = if let Some(pct) =
        value.strip_suffix('%').and_then(|n| n.parse::<f64>().ok())
    {
        Box::new(move |slot| {
            StyleProperty::MaskStopPosition(slot, stop, Dimension::Percent(pct))
        })
    } else if let Some(length) = parse_spacing_suffix(value) {
        Box::new(move |slot| {
            StyleProperty::MaskStopPosition(slot, stop, Dimension::Length(length))
        })
    } else {
        let token = value.to_string();
        Box::new(move |slot| {
            StyleProperty::MaskStopColor(slot, stop, Color::Token(token.clone()))
        })
    };
    Some(slots.iter().map(|slot| property(*slot)).collect())
}

/// Background gradients: the `bg-linear-*`/`bg-radial*`/`bg-conic-*`
/// constructors and the `from-*`/`via-*`/`to-*` stops that fill them.
///
/// The family is split across four utility names because that is how
/// Tailwind splits it -- a constructor paints nothing without stops and
/// stops paint nothing without a constructor -- so each contributes its
/// own property and `hozo_web::css::gradient_value` joins them. Which
/// also means a one-utility conformance check can say nothing about any
/// of it: every one of these 971 catalogue entries is composition-only.
fn expand_gradient(token: &str) -> Option<Vec<StyleProperty>> {
    if token == "bg-none" {
        return Some(vec![StyleProperty::BackgroundImageNone]);
    }
    if let Some(props) = gradient_stop(token) {
        return Some(props);
    }
    // The interpolation space is a *modifier* on the constructor, not a
    // utility of its own: `bg-linear-to-r/srgb`.
    let (name, modifier) = match token.split_once('/') {
        Some((name, modifier)) => (name, Some(modifier)),
        None => (token, None),
    };
    // Tailwind's default, and not sRGB: v4 interpolates gradients in
    // Oklab, which is why a red-to-blue ramp doesn't go through grey.
    let interpolation = match modifier {
        None => "in oklab".to_string(),
        Some(m) => interpolation_space(m)?,
    };

    let (kind, position) = if let Some(rest) = name.strip_prefix("bg-linear-") {
        (GradientKind::Linear, linear_position(rest)?)
    } else if let Some(rest) = name.strip_prefix("bg-gradient-") {
        // v3's spelling of the same utility, still generated by v4.
        (GradientKind::Linear, linear_position(rest)?)
    } else if name == "bg-radial" {
        (GradientKind::Radial, String::new())
    } else if let Some(rest) = name.strip_prefix("bg-conic-") {
        (GradientKind::Conic, format!("from {}deg", rest.parse::<f64>().ok()?))
    } else if name == "bg-conic" {
        (GradientKind::Conic, String::new())
    } else {
        return None;
    };

    let prelude = if position.is_empty() {
        interpolation
    } else {
        format!("{position} {interpolation}")
    };
    Some(vec![StyleProperty::Gradient(kind, prelude)])
}

/// `to-r`, `to-tl`, or a bare angle in degrees.
fn linear_position(rest: &str) -> Option<String> {
    if let Some(side) = rest.strip_prefix("to-") {
        return Some(
            match side {
                "t" => "to top",
                "tr" => "to top right",
                "r" => "to right",
                "br" => "to bottom right",
                "b" => "to bottom",
                "bl" => "to bottom left",
                "l" => "to left",
                "tl" => "to top left",
                _ => return None,
            }
            .to_string(),
        );
    }
    Some(format!("{}deg", rest.parse::<f64>().ok()?))
}

/// The `/…` modifier, as the CSS interpolation clause it names.
///
/// Two shapes in one list: a colour space, and a hue-interpolation method
/// that implies one. `bg-linear-to-r/longer` is `in oklch longer hue` --
/// the space comes along with the method, because interpolating hue the
/// long way round is only defined in a polar space.
fn interpolation_space(modifier: &str) -> Option<String> {
    Some(match modifier {
        "srgb" | "srgb-linear" | "hsl" | "hwb" | "lab" | "lch" | "oklab" | "oklch"
        | "display-p3" | "xyz" | "xyz-d50" | "xyz-d65" => format!("in {modifier}"),
        "shorter" | "longer" | "increasing" | "decreasing" => {
            format!("in oklch {modifier} hue")
        }
        _ => return None,
    })
}

/// `from-*` / `via-*` / `to-*`: a colour or a position on one stop.
fn gradient_stop(token: &str) -> Option<Vec<StyleProperty>> {
    let (stop, rest) = if let Some(rest) = token.strip_prefix("from-") {
        (GradientStop::From, rest)
    } else if let Some(rest) = token.strip_prefix("via-") {
        (GradientStop::Via, rest)
    } else if let Some(rest) = token.strip_prefix("to-") {
        (GradientStop::To, rest)
    } else {
        return None;
    };
    // A percentage is a position and everything else is a colour, which is
    // the same polarity the mask stops use -- `from-nonsense` is a
    // palette token that resolves to nothing, not a stop at `nonsense`.
    if let Some(percent) = rest.strip_suffix('%').and_then(|n| n.parse::<f64>().ok()) {
        return Some(vec![StyleProperty::GradientStopPosition(
            stop,
            Dimension::Percent(percent),
        )]);
    }
    Some(vec![StyleProperty::GradientStopColor(
        stop,
        Color::Token(rest.to_string()),
    )])
}

/// Tailwind's axis abbreviations. `x`/`y` name two slots each.
pub(crate) fn mask_slots(axis: &str) -> Option<Vec<MaskSlot>> {
    Some(match axis {
        "t" => vec![MaskSlot::Top],
        "r" => vec![MaskSlot::Right],
        "b" => vec![MaskSlot::Bottom],
        "l" => vec![MaskSlot::Left],
        "x" => vec![MaskSlot::Left, MaskSlot::Right],
        "y" => vec![MaskSlot::Bottom, MaskSlot::Top],
        "linear" => vec![MaskSlot::Linear],
        "radial" => vec![MaskSlot::Radial],
        "conic" => vec![MaskSlot::Conic],
        _ => return None,
    })
}

fn radial_position(suffix: &str) -> Option<&'static str> {
    Some(match suffix {
        "center" => "center",
        "top" => "top",
        "bottom" => "bottom",
        "left" => "left",
        "right" => "right",
        "top-left" => "left top",
        "top-right" => "right top",
        "bottom-left" => "left bottom",
        "bottom-right" => "right bottom",
        _ => return None,
    })
}

fn leak_size(size: &str) -> &'static str {
    match size {
        "closest-side" => "closest-side",
        "closest-corner" => "closest-corner",
        "farthest-side" => "farthest-side",
        _ => "farthest-corner",
    }
}

/// The `mask-*` utilities that are one property set to one keyword.
///
/// Deliberately a table rather than nested prefix matching: the family has
/// no structure to exploit -- `mask-center` is a position, `mask-cover` a
/// size, `mask-alpha` a mode -- so anything cleverer would just be a table
/// in disguise with more room for the wrong arm to win.
///
/// The gradient half of `mask-*` (`mask-t-from-*`, `mask-linear-*`, ~5,900
/// candidates) is not here: those compose into a slot-based `mask-image`
/// list and are a separate piece of work.
fn expand_mask(token: &str) -> Option<StyleProperty> {
    Some(match token {
        "mask-none" => StyleProperty::MaskImageNone,
        "mask-clip-border" => StyleProperty::MaskClip("border-box"),
        "mask-clip-content" => StyleProperty::MaskClip("content-box"),
        "mask-clip-fill" => StyleProperty::MaskClip("fill-box"),
        "mask-clip-padding" => StyleProperty::MaskClip("padding-box"),
        "mask-clip-stroke" => StyleProperty::MaskClip("stroke-box"),
        "mask-clip-view" => StyleProperty::MaskClip("view-box"),
        "mask-no-clip" => StyleProperty::MaskClip("no-clip"),
        "mask-origin-border" => StyleProperty::MaskOrigin("border-box"),
        "mask-origin-content" => StyleProperty::MaskOrigin("content-box"),
        "mask-origin-fill" => StyleProperty::MaskOrigin("fill-box"),
        "mask-origin-padding" => StyleProperty::MaskOrigin("padding-box"),
        "mask-origin-stroke" => StyleProperty::MaskOrigin("stroke-box"),
        "mask-origin-view" => StyleProperty::MaskOrigin("view-box"),
        "mask-alpha" => StyleProperty::MaskMode("alpha"),
        "mask-luminance" => StyleProperty::MaskMode("luminance"),
        "mask-match" => StyleProperty::MaskMode("match-source"),
        "mask-type-alpha" => StyleProperty::MaskType("alpha"),
        "mask-type-luminance" => StyleProperty::MaskType("luminance"),
        "mask-auto" => StyleProperty::MaskSize("auto"),
        "mask-contain" => StyleProperty::MaskSize("contain"),
        "mask-cover" => StyleProperty::MaskSize("cover"),
        "mask-center" => StyleProperty::MaskPosition("center"),
        "mask-top" => StyleProperty::MaskPosition("top"),
        "mask-bottom" => StyleProperty::MaskPosition("bottom"),
        "mask-left" => StyleProperty::MaskPosition("left"),
        "mask-right" => StyleProperty::MaskPosition("right"),
        "mask-top-left" => StyleProperty::MaskPosition("left top"),
        "mask-top-right" => StyleProperty::MaskPosition("right top"),
        "mask-bottom-left" => StyleProperty::MaskPosition("left bottom"),
        "mask-bottom-right" => StyleProperty::MaskPosition("right bottom"),
        "mask-repeat" => StyleProperty::MaskRepeat("repeat"),
        "mask-no-repeat" => StyleProperty::MaskRepeat("no-repeat"),
        "mask-repeat-x" => StyleProperty::MaskRepeat("repeat-x"),
        "mask-repeat-y" => StyleProperty::MaskRepeat("repeat-y"),
        "mask-repeat-round" => StyleProperty::MaskRepeat("round"),
        "mask-repeat-space" => StyleProperty::MaskRepeat("space"),
        _ => return None,
    })
}

/// `scroll-m*`/`scroll-p*` (optionally negated) and `scroll-smooth`.
///
/// Regular enough to be one function: eleven edges, two families, and the
/// same spacing scale everything else uses.
fn expand_scroll(token: &str) -> Option<Vec<StyleProperty>> {
    let rest = token.strip_prefix("scroll-")?;

    // Both values of the property, handled here rather than in
    // `KEYWORD_UTILITIES` -- splitting them would put `scroll-smooth` in a
    // variant and `scroll-auto` in a `Keyword`, and `dedupe_key` can't see
    // that those are the same declaration, so writing both would emit both.
    if let Some(behavior) = match rest {
        "smooth" => Some("smooth"),
        "auto" => Some("auto"),
        _ => None,
    } {
        return Some(vec![StyleProperty::ScrollBehavior(behavior)]);
    }

    let family = rest.chars().next()?;
    let (edge_part, value) = rest.get(1..)?.split_once('-')?;
    let edge = edge_keyword(edge_part)?;
    let length = parse_spacing_suffix(value)?;

    Some(match family {
        'm' => vec![StyleProperty::ScrollMargin(edge, length)],
        // `scroll-padding` takes no negative, which `negated` enforces by
        // having no arm for it.
        'p' => vec![StyleProperty::ScrollPadding(edge, length)],
        _ => return None,
    })
}

/// Tailwind's edge abbreviations, shared by every per-side family.
fn edge_keyword(suffix: &str) -> Option<Edge> {
    Some(match suffix {
        "" => Edge::All,
        "t" => Edge::Top,
        "r" => Edge::Right,
        "b" => Edge::Bottom,
        "l" => Edge::Left,
        "x" => Edge::Inline,
        "y" => Edge::Block,
        "s" => Edge::InlineStart,
        "e" => Edge::InlineEnd,
        "bs" => Edge::BlockStart,
        "be" => Edge::BlockEnd,
        _ => return None,
    })
}

/// The colour families that are a single property with no keyword forms
/// worth special-casing, plus the two that carry a width/style alongside.
fn expand_paint(token: &str) -> Option<Vec<StyleProperty>> {
    if let Some(rest) = token.strip_prefix("stroke-") {
        // `stroke-2` is a width; SVG stroke-width is unitless.
        if let Ok(n) = rest.parse::<f64>() {
            return Some(vec![StyleProperty::StrokeWidth(n)]);
        }
        if is_paint_keyword(rest) {
            return None;
        }
        return Some(vec![StyleProperty::Stroke(Color::Token(rest.to_string()))]);
    }
    if let Some(rest) = token.strip_prefix("decoration-") {
        if let Ok(px) = rest.parse::<f64>() {
            return Some(vec![StyleProperty::TextDecorationThickness(Length::Px(px))]);
        }
        if let Some(style) = decoration_style_keyword(rest) {
            return Some(vec![StyleProperty::TextDecorationStyle(style)]);
        }
        // Thickness keywords Hozo doesn't lower. Declining leaves them
        // unsupported; falling through would read them as colours named
        // `auto` and `from-font`.
        if matches!(rest, "auto" | "from-font") {
            return None;
        }
        return Some(vec![StyleProperty::TextDecorationColor(Color::Token(rest.to_string()))]);
    }
    let (prefix, make): (&str, fn(Color) -> StyleProperty) = if token.starts_with("fill-") {
        ("fill-", StyleProperty::Fill)
    } else if token.starts_with("accent-") {
        ("accent-", StyleProperty::AccentColor)
    } else if token.starts_with("caret-") {
        ("caret-", StyleProperty::CaretColor)
    } else if token.starts_with("placeholder-") {
        ("placeholder-", StyleProperty::PlaceholderColor)
    } else {
        return None;
    };
    let rest = token.strip_prefix(prefix)?;
    if is_paint_keyword(rest) {
        return None;
    }
    Some(vec![make(Color::Token(rest.to_string()))])
}

/// Suffixes in these families that are CSS keywords rather than colours:
/// `fill-none` is the SVG "don't paint" value, `accent-auto` hands the
/// control back to the UA. Declining keeps them honestly unsupported rather
/// than compiling to a colour named `none`.
fn is_paint_keyword(suffix: &str) -> bool {
    matches!(suffix, "none" | "auto")
}

/// `outline*`: width (which also implies a style, as Tailwind does), an
/// explicit style, a colour, or an offset.
fn expand_outline(token: &str) -> Option<Vec<StyleProperty>> {
    let rest = token.strip_prefix("outline")?;
    if rest.is_empty() {
        // Bare `outline` is 1px solid.
        return Some(vec![
            StyleProperty::OutlineStyle(BorderStyle::Solid),
            StyleProperty::OutlineWidth(Length::Px(1.0)),
        ]);
    }
    let suffix = rest.strip_prefix('-')?;

    if let Some(offset) = suffix.strip_prefix("offset-") {
        return offset
            .parse::<f64>()
            .ok()
            .map(|px| vec![StyleProperty::OutlineOffset(Length::Px(px))]);
    }
    // Before the style keywords, which would read `hidden` as
    // `BorderStyle::Hidden`. `outline-style: hidden` is not valid CSS --
    // an outline takes `auto | none | <border-style except hidden>` --
    // and Tailwind writes `none` here. It also adds a forced-colors
    // branch restoring a transparent outline, which Hozo doesn't emit:
    // that is a real gap, but a smaller one than an invalid declaration.
    //
    // This arm existed before and sat *after* the keyword match, so it
    // never ran. Nothing caught it because the rule Tailwind writes for
    // `outline-hidden` contains a nested at-rule, and the harness's rule
    // extractor used to drop any rule shaped like that -- the candidate
    // read as "Tailwind emits nothing for this" and left the denominator.
    if suffix == "hidden" {
        return Some(vec![StyleProperty::OutlineStyle(BorderStyle::None)]);
    }
    if let Some(style) = border_style_keyword(suffix) {
        return Some(vec![StyleProperty::OutlineStyle(style)]);
    }
    if let Ok(px) = suffix.parse::<f64>() {
        return Some(vec![
            StyleProperty::OutlineStyle(BorderStyle::Solid),
            StyleProperty::OutlineWidth(Length::Px(px)),
        ]);
    }
    Some(vec![StyleProperty::OutlineColor(Color::Token(suffix.to_string()))])
}

/// `divide-*`: the border between an element's children. Shares the
/// child-scoped rule mechanism with `space-*`.
fn expand_divide(token: &str) -> Option<Vec<StyleProperty>> {
    let suffix = token.strip_prefix("divide-")?;

    if let Some(width) = suffix.strip_prefix("x") {
        if let Some(length) = divide_width(width) {
            return Some(vec![StyleProperty::DivideX(Dimension::Length(length))]);
        }
    }
    if let Some(width) = suffix.strip_prefix("y") {
        if let Some(length) = divide_width(width) {
            return Some(vec![StyleProperty::DivideY(Dimension::Length(length))]);
        }
    }
    if let Some(style) = border_style_keyword(suffix) {
        return Some(vec![StyleProperty::DivideStyle(style)]);
    }
    // `divide-x-reverse` only flips which edge the border sits on, through
    // a custom property Hozo doesn't emit; it paints nothing standalone.
    if suffix.ends_with("-reverse") {
        return None;
    }
    Some(vec![StyleProperty::DivideColor(Color::Token(suffix.to_string()))])
}

/// The width half of `divide-x*`/`divide-y*`: empty means 1px.
fn divide_width(suffix: &str) -> Option<Length> {
    match suffix {
        "" => Some(Length::Px(1.0)),
        rest => rest.strip_prefix('-')?.parse::<f64>().ok().map(Length::Px),
    }
}

fn decoration_style_keyword(suffix: &str) -> Option<DecorationStyle> {
    Some(match suffix {
        "solid" => DecorationStyle::Solid,
        "double" => DecorationStyle::Double,
        "dotted" => DecorationStyle::Dotted,
        "dashed" => DecorationStyle::Dashed,
        "wavy" => DecorationStyle::Wavy,
        _ => return None,
    })
}

fn border_style_keyword(suffix: &str) -> Option<BorderStyle> {
    Some(match suffix {
        "solid" => BorderStyle::Solid,
        "dashed" => BorderStyle::Dashed,
        "dotted" => BorderStyle::Dotted,
        "double" => BorderStyle::Double,
        "hidden" => BorderStyle::Hidden,
        "none" => BorderStyle::None,
        _ => return None,
    })
}

/// `ring*` / `inset-ring*`: a width, or the colour that width renders in.
///
/// A colour on its own emits nothing, which is correct rather than a gap --
/// it only means something once a width is present, and the two compose in
/// the backend (see `StyleProperty::RingWidth`). Tailwind behaves the same
/// way; standalone `ring-red-500` produces no declarations either, only a
/// custom property.
fn parse_ring(token: &str) -> Option<StyleProperty> {
    let (inset, rest) = match token.strip_prefix("inset-ring") {
        Some(rest) => (true, rest),
        None => (false, token.strip_prefix("ring")?),
    };
    // `ring-offset-*` is a layer of its own with its own colour and width,
    // not a spelling of the ring -- reading it as one would give a ring
    // colour called `offset-red-500`. There is no inset form.
    if let Some(suffix) = rest.strip_prefix("-offset-") {
        if inset {
            return None;
        }
        return Some(match suffix.parse::<f64>() {
            Ok(px) => StyleProperty::RingOffsetWidth(Length::Px(px)),
            Err(_) => StyleProperty::RingOffsetColor(Color::Token(suffix.to_string())),
        });
    }
    let suffix = match rest {
        // Bare `ring` / `inset-ring` is 1px.
        "" => return Some(width_prop(inset, Length::Px(1.0))),
        rest => rest.strip_prefix('-')?,
    };
    match suffix.parse::<f64>() {
        Ok(px) => Some(width_prop(inset, Length::Px(px))),
        Err(_) => Some(if inset {
            StyleProperty::InsetRingColor(Color::Token(suffix.to_string()))
        } else {
            StyleProperty::RingColor(Color::Token(suffix.to_string()))
        }),
    }
}

fn width_prop(inset: bool, width: Length) -> StyleProperty {
    if inset {
        StyleProperty::InsetRingWidth(width)
    } else {
        StyleProperty::RingWidth(width)
    }
}

/// `inset-<value>` and `inset-<side>-<value>`, each optionally negated.
///
/// Bare `inset-*` sets all four sides, so it stays four physical
/// properties; the axis and logical forms map to the single CSS property
/// Tailwind emits for each.
fn expand_inset(token: &str) -> Option<Vec<StyleProperty>> {
    let rest = token.strip_prefix("inset-")?;

    let (side, value) = match rest.split_once('-') {
        Some((side, value)) if matches!(side, "x" | "y" | "s" | "e" | "bs" | "be") => {
            (Some(side), value)
        }
        _ => (None, rest),
    };
    let length = parse_dimension_suffix(value)?;

    Some(match side {
        Some("x") => vec![StyleProperty::InsetInline(length)],
        Some("y") => vec![StyleProperty::InsetBlock(length)],
        Some("s") => vec![StyleProperty::InsetInlineStart(length)],
        Some("e") => vec![StyleProperty::InsetInlineEnd(length)],
        Some("bs") => vec![StyleProperty::InsetBlockStart(length)],
        Some("be") => vec![StyleProperty::InsetBlockEnd(length)],
        _ => vec![
            StyleProperty::InsetTop(length.clone()),
            StyleProperty::InsetRight(length.clone()),
            StyleProperty::InsetBottom(length.clone()),
            StyleProperty::InsetLeft(length),
        ],
    })
}

/// `border-<side>-<color>`, for every side Tailwind offers.
///
/// The width forms (`border-t-2`) matched an earlier arm, so a side here is
/// always followed by a colour token. Until 2026-08-15 this fell through to
/// the plain colour path and compiled `border-b-red-500` to
/// `border-color: var(--hozo-color-b-red-500)` -- the wrong property, on
/// all four sides, from a token that isn't a colour name.
fn parse_border_side_color(suffix: &str) -> Option<StyleProperty> {
    let (side, token) = suffix.split_once('-')?;
    // A number is a *width* on that side (`border-x-2`), not a colour. The
    // physical sides' widths matched an earlier arm; the logical and axis
    // ones aren't lowered yet, so declining here lets the guard below
    // refuse them by name instead of inventing a colour called "2".
    if token.parse::<f64>().is_ok() {
        return None;
    }
    let color = Color::Token(token.to_string());
    Some(match side {
        "t" => StyleProperty::BorderTopColor(color),
        "r" => StyleProperty::BorderRightColor(color),
        "b" => StyleProperty::BorderBottomColor(color),
        "l" => StyleProperty::BorderLeftColor(color),
        "x" => StyleProperty::BorderInlineColor(color),
        "y" => StyleProperty::BorderBlockColor(color),
        "s" => StyleProperty::BorderInlineStartColor(color),
        "e" => StyleProperty::BorderInlineEndColor(color),
        "bs" => StyleProperty::BorderBlockStartColor(color),
        "be" => StyleProperty::BorderBlockEndColor(color),
        _ => return None,
    })
}

#[derive(Clone, Copy)]
enum ColorFamily {
    Background,
    Text,
    Border,
}

/// Whether a `bg-`/`text-`/`border-` suffix is Tailwind's name for
/// something that isn't a colour.
///
/// These families end in a catch-all: whatever the earlier arms declined is
/// treated as a colour token, and an unrecognized one becomes
/// `var(--hozo-color-<token>)` so a project's own theme colour still
/// reaches CSS. That is the right behaviour for `bg-brand-primary`, and
/// quietly wrong for everything else -- `bg-auto` is a background *size*,
/// and it was compiling to `background-color: var(--hozo-color-auto)`, a
/// custom property nothing defines. Inert output, and Hozo claiming the
/// utility while producing it.
///
/// So the catch-all is now guarded by the list below, derived from
/// Tailwind's own class list by asking which entries in each family it does
/// *not* give a colour property to (523 candidates did this). Matching one
/// means "not supported yet", not "not a colour utility Hozo will ever
/// have" -- these are the implementation targets.
fn is_non_color_suffix(family: ColorFamily, suffix: &str) -> bool {
    let head = suffix.split('-').next().unwrap_or(suffix);
    match family {
        // background-size / -position / -repeat / -attachment / -clip /
        // -origin / -blend-mode, and the gradient constructors.
        ColorFamily::Background => matches!(
            head,
            "auto"
                | "blend"
                | "bottom"
                | "center"
                | "clip"
                | "conic"
                | "contain"
                | "cover"
                | "fixed"
                | "left"
                | "linear"
                | "local"
                | "no"
                | "none"
                | "origin"
                | "radial"
                | "repeat"
                | "right"
                | "scroll"
                | "top"
        ),
        // text-wrap / text-align's logical keywords / text-shadow.
        ColorFamily::Text => {
            matches!(head, "balance" | "end" | "justify" | "nowrap" | "pretty" | "shadow" | "start" | "wrap")
        }
        ColorFamily::Border => {
            // Table borders, plus the two border-styles the width/style arms
            // above don't recognize.
            if matches!(head, "collapse" | "separate" | "spacing") {
                return true;
            }
            // Anything still carrying a side keyword by the time it reaches
            // the colour catch-all is unsupported. The widths that *are*
            // supported (`border-t-4`) matched an earlier arm and never get
            // here; what's left is per-side colours, which would otherwise
            // become `border-color` -- all four sides, from a token that
            // isn't a colour name (`border-b-red-500` was compiling to
            // `border-color: var(--hozo-color-b-red-500)`) -- plus the
            // logical/axis widths Hozo doesn't lower yet.
            matches!(head, "t" | "r" | "b" | "l" | "x" | "y" | "s" | "e" | "bs" | "be")
        }
    }
}

/// Tailwind's `--radius-*` scale, in px (its own values are rem at the
/// default 16px root). Bare `rounded` is 0.25rem, which is *not* the same
/// as `rounded-sm` in v4 -- they happen to share a value here but are
/// separate scale entries.
fn parse_border_radius(token: &str) -> Option<Radius> {
    radius_from_suffix(token.strip_prefix("rounded")?.strip_prefix('-').unwrap_or(""))
}

/// The size half of a `rounded-*` utility, with the corner (if any) already
/// stripped. An empty suffix is bare `rounded`.
fn radius_from_suffix(suffix: &str) -> Option<Radius> {
    // Kept as an intent rather than a number -- see `hozo_ir::Radius`.
    if suffix == "full" {
        return Some(Radius::Full);
    }
    let px = match suffix {
        "" => 4.0,
        "none" => 0.0,
        "xs" => 2.0,
        "sm" => 4.0,
        "md" => 6.0,
        "lg" => 8.0,
        "xl" => 12.0,
        "2xl" => 16.0,
        "3xl" => 24.0,
        "4xl" => 32.0,
        _ => return None,
    };
    Some(Radius::Length(Length::Px(px)))
}

/// `rounded-<corner>-<size>`, where a corner may be a single corner, one
/// edge (two corners), or their logical equivalents.
fn expand_border_radius(token: &str) -> Option<Vec<StyleProperty>> {
    let rest = token.strip_prefix("rounded")?.strip_prefix('-')?;
    // `rounded-lg` has no corner part; `rounded-t-lg` does. The corner is
    // never a valid size and vice versa, so trying the whole suffix as a
    // size first disambiguates without a lookahead.
    let (corner, size) = match rest.split_once('-') {
        Some((corner, size)) if radius_from_suffix(size).is_some() => (corner, size),
        _ => return None,
    };
    let r = radius_from_suffix(size)?;

    Some(match corner {
        "t" => vec![
            StyleProperty::BorderTopLeftRadius(r),
            StyleProperty::BorderTopRightRadius(r),
        ],
        "r" => vec![
            StyleProperty::BorderTopRightRadius(r),
            StyleProperty::BorderBottomRightRadius(r),
        ],
        "b" => vec![
            StyleProperty::BorderBottomRightRadius(r),
            StyleProperty::BorderBottomLeftRadius(r),
        ],
        "l" => vec![
            StyleProperty::BorderTopLeftRadius(r),
            StyleProperty::BorderBottomLeftRadius(r),
        ],
        "s" => vec![
            StyleProperty::BorderStartStartRadius(r),
            StyleProperty::BorderEndStartRadius(r),
        ],
        "e" => vec![
            StyleProperty::BorderStartEndRadius(r),
            StyleProperty::BorderEndEndRadius(r),
        ],
        "tl" => vec![StyleProperty::BorderTopLeftRadius(r)],
        "tr" => vec![StyleProperty::BorderTopRightRadius(r)],
        "br" => vec![StyleProperty::BorderBottomRightRadius(r)],
        "bl" => vec![StyleProperty::BorderBottomLeftRadius(r)],
        "ss" => vec![StyleProperty::BorderStartStartRadius(r)],
        "se" => vec![StyleProperty::BorderStartEndRadius(r)],
        "es" => vec![StyleProperty::BorderEndStartRadius(r)],
        "ee" => vec![StyleProperty::BorderEndEndRadius(r)],
        _ => return None,
    })
}

/// Tailwind's named `--leading-*` scale: unitless multipliers of the
/// element's own font size, unlike the numeric `leading-<n>` scale which is
/// the spacing scale in pixels.
fn parse_named_leading(suffix: &str) -> Option<f64> {
    Some(match suffix {
        "none" => 1.0,
        "tight" => 1.25,
        "snug" => 1.375,
        "normal" => 1.5,
        "relaxed" => 1.625,
        "loose" => 2.0,
        _ => return None,
    })
}

/// Tailwind's `--tracking-*` scale, in em.
fn parse_tracking(suffix: &str) -> Option<f64> {
    Some(match suffix {
        "tighter" => -0.05,
        "tight" => -0.025,
        "normal" => 0.0,
        "wide" => 0.025,
        "wider" => 0.05,
        "widest" => 0.1,
        _ => return None,
    })
}

/// Margin value: the spacing scale plus `auto`, which is what makes
/// `mx-auto` (centre a fixed-width box) work. Padding has no `auto`.
fn parse_margin_suffix(suffix: &str) -> Option<Dimension> {
    if suffix == "auto" {
        return Some(Dimension::Auto);
    }
    parse_spacing_suffix(suffix).map(Dimension::Length)
}

/// Single-side margins (`mt-2`, `ms-auto`, ...). The multi-side forms
/// (`m-`, `mx-`, `my-`) expand to several properties and live in
/// `expand_base_utility`.
fn parse_single_margin(token: &str) -> Option<StyleProperty> {
    let (prefix, rest) = token.split_once('-')?;
    let value = parse_margin_suffix(rest)?;
    match prefix {
        "mt" => Some(StyleProperty::MarginTop(value)),
        "mr" => Some(StyleProperty::MarginRight(value)),
        "mb" => Some(StyleProperty::MarginBottom(value)),
        "ml" => Some(StyleProperty::MarginLeft(value)),
        "ms" => Some(StyleProperty::MarginInlineStart(value)),
        "me" => Some(StyleProperty::MarginInlineEnd(value)),
        _ => None,
    }
}

/// Tailwind's `--shadow-*` scale, verbatim. Emitted as a composed CSS
/// string because React Native's `boxShadow` accepts one too, so both
/// backends can carry the same text.
///
/// Tailwind's own `box-shadow` declaration also splices in its ring and
/// inset-ring registers, but those are `0 0 #0000` (fully transparent, a
/// no-op) unless a `ring-*` utility is present -- which Hozo doesn't
/// support -- so only the shadow itself is emitted.
fn parse_shadow(token: &str) -> Option<&'static str> {
    Some(match token {
        "shadow-2xs" => "0 1px rgb(0 0 0 / 0.05)",
        "shadow-xs" => "0 1px 2px 0 rgb(0 0 0 / 0.05)",
        "shadow-sm" | "shadow" => "0 1px 3px 0 rgb(0 0 0 / 0.1), 0 1px 2px -1px rgb(0 0 0 / 0.1)",
        "shadow-md" => "0 4px 6px -1px rgb(0 0 0 / 0.1), 0 2px 4px -2px rgb(0 0 0 / 0.1)",
        "shadow-lg" => "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)",
        "shadow-xl" => "0 20px 25px -5px rgb(0 0 0 / 0.1), 0 8px 10px -6px rgb(0 0 0 / 0.1)",
        "shadow-2xl" => "0 25px 50px -12px rgb(0 0 0 / 0.25)",
        "shadow-inner" => "inset 0 2px 4px 0 rgb(0 0 0 / 0.05)",
        "shadow-none" => "none",
        _ => return None,
    })
}

fn parse_blur(token: &str) -> Option<f64> {
    Some(match token {
        "blur-xs" => 4.0,
        "blur-sm" | "blur" => 8.0,
        "blur-md" => 12.0,
        "blur-lg" => 16.0,
        "blur-xl" => 24.0,
        "blur-2xl" => 40.0,
        "blur-3xl" => 64.0,
        _ => return None,
    })
}

/// One function of a `filter` chain, from the utility that names it.
///
/// `Some((function, ""))` is the `-none` form, which clears that function's
/// slot rather than setting it -- Tailwind writes `--tw-blur: ;`, an empty
/// register, so the slot contributes nothing to the composed value.
fn parse_filter_function(token: &str) -> Option<(FilterFunction, String)> {
    if token == "filter-none" {
        return Some((FilterFunction::None, String::new()));
    }
    if let Some(px) = parse_blur(token) {
        return Some((FilterFunction::Blur, format!("blur({px}px)")));
    }
    if token == "blur-none" {
        return Some((FilterFunction::Blur, String::new()));
    }

    // The percentage-valued functions. Each has a bare form meaning 100%
    // (`grayscale` is a full conversion) and a numbered form.
    const PERCENT: &[(&str, FilterFunction)] = &[
        ("brightness", FilterFunction::Brightness),
        ("contrast", FilterFunction::Contrast),
        ("grayscale", FilterFunction::Grayscale),
        ("invert", FilterFunction::Invert),
        ("saturate", FilterFunction::Saturate),
        ("sepia", FilterFunction::Sepia),
        ("opacity", FilterFunction::Opacity),
    ];
    for (name, function) in PERCENT {
        if token == *name {
            return Some((*function, format!("{name}(100%)")));
        }
        if let Some(rest) = token.strip_prefix(&format!("{name}-")) {
            if rest == "none" {
                return Some((*function, String::new()));
            }
            let pct: f64 = rest.parse().ok()?;
            return Some((*function, format!("{name}({pct}%)")));
        }
    }
    // Tailwind's drop-shadow scale. The colour is a register there
    // (`--tw-drop-shadow-color`) with these as its defaults; Hozo resolves
    // the composition, so the default is written in.
    if let Some(rest) = token.strip_prefix("drop-shadow") {
        let shadow = match rest.trim_start_matches('-') {
            "xs" => "drop-shadow(0 1px 1px rgb(0 0 0 / 0.05))",
            "sm" => "drop-shadow(0 1px 2px rgb(0 0 0 / 0.15))",
            "md" => "drop-shadow(0 3px 3px rgb(0 0 0 / 0.12))",
            "lg" => "drop-shadow(0 4px 4px rgb(0 0 0 / 0.15))",
            "xl" => "drop-shadow(0 9px 7px rgb(0 0 0 / 0.1))",
            "2xl" => "drop-shadow(0 25px 25px rgb(0 0 0 / 0.15))",
            "none" => "",
            _ => return None,
        };
        return Some((FilterFunction::DropShadow, shadow.to_string()));
    }
    if token == "hue-rotate" {
        return Some((FilterFunction::HueRotate, "hue-rotate(0deg)".to_string()));
    }
    if let Some(rest) = token.strip_prefix("hue-rotate-") {
        let degrees: f64 = rest.parse().ok()?;
        return Some((FilterFunction::HueRotate, format!("hue-rotate({degrees}deg)")));
    }
    None
}

/// `filter-*` and `backdrop-*`, which are the same chain applied to the
/// element and to what's behind it.
fn expand_filter(token: &str) -> Option<Vec<StyleProperty>> {
    if let Some(rest) = token.strip_prefix("backdrop-") {
        // `backdrop-filter-none` reaches here as `filter-none`.
        let (function, value) = parse_filter_function(rest)?;
        return Some(vec![StyleProperty::BackdropFilter(function, value)]);
    }
    let (function, value) = parse_filter_function(token)?;
    // Bare `opacity-*` is the CSS property, not a filter function -- only
    // its backdrop form exists as a filter.
    if function == FilterFunction::Opacity {
        return None;
    }
    Some(vec![StyleProperty::Filter(function, value)])
}

/// `rotate-<deg>`, `scale-<pct>`, `translate-x-<n>`, `translate-y-<n>`,
/// each optionally negated with a leading `-`.
fn parse_transform(token: &str) -> Option<StyleProperty> {
    let (negative, token) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    if let Some(rest) = token.strip_prefix("rotate-") {
        let degrees: f64 = rest.parse().ok()?;
        return Some(StyleProperty::Rotate(Angle::Deg(signed(degrees, negative))));
    }
    // `translate-x-1/2` is the centring idiom, so these take the wider
    // `Dimension` rather than a pixel length.
    if let Some(rest) = token.strip_prefix("translate-x-") {
        return parse_dimension_suffix(rest).map(StyleProperty::TranslateX);
    }
    if let Some(rest) = token.strip_prefix("translate-y-") {
        return parse_dimension_suffix(rest).map(StyleProperty::TranslateY);
    }
    None
}

/// Applies a `-` prefix, keeping zero unsigned.
///
/// IEEE negation of `0.0` gives `-0.0`, which Rust prints as `-0` -- so
/// `-scale-0` emitted `scale: -0% -0%` where Tailwind emits `0 0`. The
/// values behave identically in CSS; the strings don't, and a differential
/// test compares strings.
fn signed(value: f64, negative: bool) -> f64 {
    let result = if negative { -value } else { value };
    if result == 0.0 {
        0.0
    } else {
        result
    }
}

/// The grid track and placement utilities: `grid-cols-*`/`grid-rows-*`,
/// `col-start-*`/`col-end-*`/`row-start-*`/`row-end-*`, and the
/// `col-span-*`/`row-span-*` shorthands.
///
/// Prefix order matters here in a way it doesn't elsewhere: `col-span-` and
/// `col-start-` both begin with `col-`, so the shorthands are matched
/// before the bare-line form that would otherwise swallow them.
fn parse_grid_placement(token: &str) -> Option<StyleProperty> {
    fn tracks(suffix: &str) -> Option<GridTracks> {
        Some(match suffix {
            "none" => GridTracks::None,
            "subgrid" => GridTracks::Subgrid,
            _ => GridTracks::Count(suffix.parse().ok()?),
        })
    }
    fn line(suffix: &str) -> Option<GridLine> {
        Some(match suffix {
            "auto" => GridLine::Auto,
            _ => GridLine::Line(suffix.parse().ok()?),
        })
    }
    fn span(suffix: &str) -> Option<GridSpan> {
        Some(match suffix {
            "auto" => GridSpan::Auto,
            "full" => GridSpan::Full,
            _ => GridSpan::Span(suffix.parse().ok()?),
        })
    }

    if let Some(rest) = token.strip_prefix("grid-cols-") {
        return tracks(rest).map(StyleProperty::GridTemplateColumns);
    }
    if let Some(rest) = token.strip_prefix("grid-rows-") {
        return tracks(rest).map(StyleProperty::GridTemplateRows);
    }
    if let Some(rest) = token.strip_prefix("col-span-") {
        return span(rest).map(StyleProperty::GridColumn);
    }
    if let Some(rest) = token.strip_prefix("row-span-") {
        return span(rest).map(StyleProperty::GridRow);
    }
    if let Some(rest) = token.strip_prefix("col-start-") {
        return line(rest).map(StyleProperty::GridColumnStart);
    }
    if let Some(rest) = token.strip_prefix("col-end-") {
        return line(rest).map(StyleProperty::GridColumnEnd);
    }
    if let Some(rest) = token.strip_prefix("row-start-") {
        return line(rest).map(StyleProperty::GridRowStart);
    }
    if let Some(rest) = token.strip_prefix("row-end-") {
        return line(rest).map(StyleProperty::GridRowEnd);
    }
    // `col-auto`/`row-auto` are the shorthand, not a single edge.
    match token {
        "col-auto" => Some(StyleProperty::GridColumn(GridSpan::Auto)),
        "row-auto" => Some(StyleProperty::GridRow(GridSpan::Auto)),
        _ => None,
    }
}

/// `order-<n>`, plus the two named extremes.
///
/// `order-first`/`order-last` are Tailwind's own sentinels rather than CSS
/// keywords -- `order` has none, so "first" is spelled as a number far
/// enough out that nothing outranks it. Matching the exact values matters:
/// they're what makes `order-first` beat a hand-written `order-[-999]`.
fn parse_order(token: &str) -> Option<StyleProperty> {
    let value = match token {
        "order-first" => -9999,
        "order-last" => 9999,
        "order-none" => 0,
        _ => token.strip_prefix("order-")?.parse::<i32>().ok()?,
    };
    Some(StyleProperty::Order(value))
}

/// `columns-<n>` (a count) or `columns-<size>` (an ideal column width, from
/// the same container scale the inline sizes use).
fn parse_columns_suffix(suffix: &str) -> Option<ColumnCount> {
    if suffix == "auto" {
        return Some(ColumnCount::Auto);
    }
    if let Ok(count) = suffix.parse::<u32>() {
        return Some(ColumnCount::Count(count));
    }
    parse_inline_size_suffix(suffix).map(ColumnCount::Width)
}

/// `cursor-*`. The value is passed straight through, so the table is only
/// deciding *which names are utilities* -- accepting anything after the
/// prefix would compile `cursor-nonsense` into CSS the browser drops.
fn parse_cursor(token: &str) -> Option<&str> {
    const KEYWORDS: &[&str] = &[
        "alias", "all-scroll", "auto", "cell", "col-resize", "context-menu", "copy", "crosshair",
        "default", "e-resize", "ew-resize", "grab", "grabbing", "help", "move", "n-resize",
        "ne-resize", "nesw-resize", "no-drop", "none", "not-allowed", "ns-resize", "nw-resize",
        "nwse-resize", "pointer", "progress", "row-resize", "s-resize", "se-resize", "sw-resize",
        "text", "vertical-text", "w-resize", "wait", "zoom-in", "zoom-out",
    ];
    let keyword = token.strip_prefix("cursor-")?;
    KEYWORDS.contains(&keyword).then_some(keyword)
}

/// A size suffix that may also name Tailwind's container scale (`w-md` is
/// `--container-md`, 28rem), falling back to the ordinary dimension parser.
///
/// The scale is **inline-axis only**: `w-*`, `min-w-*`, `max-w-*`,
/// `basis-*` and the `inline-*` logical family take it, and nothing on the
/// block axis does. Tailwind emits no rule at all for `max-h-md` or
/// `h-2xl` -- containers are a measure of line length, so a named one on
/// the block axis has no meaning.
///
/// Applying it to `max-h-*` (as this did until 2026-08-16) was invisible to
/// the conformance report rather than caught by it: a candidate Tailwind
/// produces no rule for leaves the denominator, so Hozo accepting a class
/// Tailwind rejects is exactly the shape of error that report can't see.
/// `border-s/e/x/y/bs/be`, the logical-edge border widths.
///
/// Each writes the style alongside the width for the same reason the
/// physical sides do: CSS defaults `border-style` to `none`, so a width
/// alone renders nothing, and scoping the style to the same edge stops the
/// untouched edges falling back to `border-width: medium` and appearing.
fn parse_logical_border(token: &str) -> Option<Vec<StyleProperty>> {
    let rest = token.strip_prefix("border-")?;
    let (name, width) = match rest.split_once('-') {
        Some((name, value)) => (name, parse_border_width_px(value)?),
        // `border-s` with no width means 1px, as for the physical sides.
        None => (rest, Length::Px(1.0)),
    };
    let edge = match name {
        "s" => Edge::InlineStart,
        "e" => Edge::InlineEnd,
        "x" => Edge::Inline,
        "y" => Edge::Block,
        "bs" => Edge::BlockStart,
        "be" => Edge::BlockEnd,
        _ => return None,
    };
    Some(vec![
        StyleProperty::BorderLogicalWidth(edge, width),
        StyleProperty::BorderLogicalStyle(edge, BorderStyle::Solid),
    ])
}

/// The values of already-modelled properties that Hozo didn't cover.
///
/// These can't join `KEYWORD_UTILITIES`: their property already has a
/// variant, and a property reachable both ways stops deduping against
/// itself (see `StyleProperty::Keyword`). So each one extends its own
/// enum instead, through the `Css` fallthrough those enums carry for
/// exactly this -- values that are a keyword and nothing more, which the
/// Web writes verbatim and React Native has no equivalent for.
fn parse_extended_value(token: &str) -> Option<StyleProperty> {
    fn overflow(value: &str) -> Option<Overflow> {
        Some(match value {
            "visible" => Overflow::Visible,
            "hidden" => Overflow::Hidden,
            "scroll" => Overflow::Scroll,
            "auto" => Overflow::Css("auto"),
            "clip" => Overflow::Css("clip"),
            _ => return None,
        })
    }
    if let Some(rest) = token.strip_prefix("overflow-x-") {
        return overflow(rest).map(StyleProperty::OverflowX);
    }
    if let Some(rest) = token.strip_prefix("overflow-y-") {
        return overflow(rest).map(StyleProperty::OverflowY);
    }
    if let Some(rest) = token.strip_prefix("overflow-") {
        return overflow(rest).map(StyleProperty::Overflow);
    }

    // `content-*` is align-content, `justify-*` is justify-content. Both
    // take the same extra values, and `safe` is a prefix on the alignment
    // rather than a keyword of its own.
    let alignment = |rest: &str| -> Option<&'static str> {
        Some(match rest {
            "normal" => "normal",
            "center-safe" => "safe center",
            "end-safe" => "safe flex-end",
            _ => return None,
        })
    };
    // `baseline` and `stretch` have real variants -- routing them through
    // `Css` would refuse them on Native, where RN's alignment unions do
    // have both. The refusal audit caught exactly that.
    match token {
        "fixed" => return Some(StyleProperty::Position(Position::Css("fixed"))),
        "static" => return Some(StyleProperty::Position(Position::Static)),
        "sticky" => return Some(StyleProperty::Position(Position::Css("sticky"))),
        "text-start" => return Some(StyleProperty::TextAlign(TextAlign::Css("start"))),
        "text-end" => return Some(StyleProperty::TextAlign(TextAlign::Css("end"))),
        "text-justify" => return Some(StyleProperty::TextAlign(TextAlign::Css("justify"))),
        // CSS's `last baseline` aligns the *last* line's baseline, which is
        // a different rule rather than a spelling of `baseline`.
        "items-baseline-last" => return Some(StyleProperty::AlignItems(Align::Css("last baseline"))),
        "self-baseline-last" => return Some(StyleProperty::AlignSelf(AlignSelf::Css("last baseline"))),
        "rotate-none" => return Some(StyleProperty::RotateNone),
        "scale-none" => return Some(StyleProperty::ScaleNone),
        // `transform` and `transform-cpu` write the composed `transform`
        // with nothing in it -- Tailwind emits the register chain, which
        // resolves to empty. `transform-gpu` prepends a null 3D
        // translation, the old trick for forcing GPU compositing.
        "transform" | "transform-cpu" => return Some(StyleProperty::TransformEmpty),
        "transform-gpu" => return Some(StyleProperty::TransformGpu),
        "transform-none" => return Some(StyleProperty::TransformNone),
        "translate-none" => return Some(StyleProperty::TranslateNone),
        // Writes the three-value form with every axis at its default, which
        // is how Tailwind switches the z component on.
        "translate-3d" => {
            return Some(StyleProperty::TranslateZ(Dimension::Length(Length::Px(0.0))));
        }
        "decoration-auto" => return Some(StyleProperty::Keyword("text-decoration-thickness", "auto")),
        "decoration-from-font" => {
            return Some(StyleProperty::Keyword("text-decoration-thickness", "from-font"))
        }
        "fill-none" => return Some(StyleProperty::Fill(Color::Keyword("none"))),
        "stroke-none" => return Some(StyleProperty::Stroke(Color::Keyword("none"))),
        "accent-auto" => return Some(StyleProperty::AccentColor(Color::Keyword("auto"))),
        "items-baseline" => return Some(StyleProperty::AlignItems(Align::Baseline)),
        "items-stretch" => return Some(StyleProperty::AlignItems(Align::Stretch)),
        "self-baseline" => return Some(StyleProperty::AlignSelf(AlignSelf::Baseline)),
        "self-stretch" => return Some(StyleProperty::AlignSelf(AlignSelf::Stretch)),
        "content-stretch" => return Some(StyleProperty::AlignContent(Justify::Stretch)),
        "content-baseline" => return Some(StyleProperty::AlignContent(Justify::Baseline)),
        "justify-stretch" => return Some(StyleProperty::JustifyContent(Justify::Stretch)),
        "justify-baseline" => return Some(StyleProperty::JustifyContent(Justify::Baseline)),
        _ => {}
    }
    if let Some(rest) = token.strip_prefix("content-") {
        return alignment(rest).map(|v| StyleProperty::AlignContent(Justify::Css(v)));
    }
    if let Some(rest) = token.strip_prefix("justify-") {
        return alignment(rest).map(|v| StyleProperty::JustifyContent(Justify::Css(v)));
    }
    if let Some(rest) = token.strip_prefix("items-") {
        return alignment(rest).map(|v| StyleProperty::AlignItems(Align::Css(v)));
    }
    if let Some(rest) = token.strip_prefix("self-") {
        return alignment(rest).map(|v| StyleProperty::AlignSelf(AlignSelf::Css(v)));
    }

    if let Some(rest) = token.strip_prefix("object-") {
        if let Some(fit) = ["contain", "cover", "fill", "none", "scale-down"]
            .into_iter()
            .find(|f| *f == rest)
        {
            return Some(StyleProperty::ObjectFit(fit));
        }
    }
    if let Some(rest) = token.strip_prefix("select-") {
        if let Some(value) =
            ["all", "auto", "none", "text"].into_iter().find(|v| *v == rest)
        {
            return Some(StyleProperty::UserSelect(value));
        }
    }
    if let Some(rest) = token.strip_prefix("whitespace-") {
        if let Some(value) = ["pre", "pre-line", "pre-wrap", "break-spaces"]
            .into_iter()
            .find(|v| *v == rest)
        {
            return Some(StyleProperty::WhiteSpace(WhiteSpace::Css(value)));
        }
    }
    match token {
        "underline" => Some(StyleProperty::TextDecorationLine("underline")),
        "overline" => Some(StyleProperty::TextDecorationLine("overline")),
        "line-through" => Some(StyleProperty::TextDecorationLine("line-through")),
        "no-underline" => Some(StyleProperty::TextDecorationLine("none")),
        _ => None,
    }
}

/// The utilities that are exactly one CSS declaration with a fixed value.
///
/// Ninety-odd properties whose whole content is a closed list of keywords
/// Hozo neither computes nor reinterprets -- `touch-action`, `contain`,
/// `break-after`, `color-scheme`. Modelling each as its own
/// `StyleProperty` would be ninety enum arms and three hundred backend
/// arms restating the CSS spec; this states it once.
///
/// Generated from Tailwind's own output rather than written from memory:
/// each row is what `tailwindcss` compiles that class to, read through the
/// same normalizer the conformance report uses. That is why the values keep
/// details a hand-written table would smooth over -- `place-content-center-safe`
/// really is `safe center`, `origin-left` really is `0` and not `left`.
///
/// **Invariant**: no property here may also be modelled as a variant of its
/// own. `dedupe_key` tells two `Keyword`s apart by property name but cannot
/// see that `Keyword("align-items", ..)` and an `AlignItems(..)` are the
/// same declaration, so both spellings would stop overriding each other.
/// `keyword_table_avoids_the_modelled_properties` is the test that holds
/// the line.
const KEYWORD_UTILITIES: &[(&str, &str, &str)] = &[
        // Not variants, despite the `@`: these declare an element *as* a
        // container, where `@lg:` and `@container/main:` query one.
        ("@container", "container-type", "inline-size"),
        ("@container-normal", "container-type", "normal"),
        ("backface-hidden", "backface-visibility", "hidden"),
        ("backface-visible", "backface-visibility", "visible"),
        ("box-border", "box-sizing", "border-box"),
        ("box-content", "box-sizing", "content-box"),
        ("content-none", "content", "none"),
        ("flex-nowrap", "flex-wrap", "nowrap"),
        ("flex-wrap-reverse", "flex-wrap", "wrap-reverse"),
        ("flex-wrap", "flex-wrap", "wrap"),
        ("italic", "font-style", "italic"),
        ("not-italic", "font-style", "normal"),
        ("isolate", "isolation", "isolate"),
        ("isolation-auto", "isolation", "auto"),
        ("pointer-events-auto", "pointer-events", "auto"),
        ("pointer-events-none", "pointer-events", "none"),
        ("collapse", "visibility", "collapse"),
        ("invisible", "visibility", "hidden"),
        ("visible", "visibility", "visible"),
        ("font-mono", "font-family", "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace"),
        ("font-sans", "font-family", "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, 'Helvetica Neue', 'Noto Sans', Arial, sans-serif, 'Apple Color Emoji', 'Segoe UI Emoji', 'Segoe UI Symbol', 'Noto Color Emoji'"),
        ("font-serif", "font-family", "ui-serif, Georgia, Cambria, 'Times New Roman', Times, serif"),
        ("align-baseline", "vertical-align", "baseline"),
        ("align-bottom", "vertical-align", "bottom"),
        ("align-middle", "vertical-align", "middle"),
        ("align-sub", "vertical-align", "sub"),
        ("align-super", "vertical-align", "super"),
        ("align-text-bottom", "vertical-align", "text-bottom"),
        ("align-text-top", "vertical-align", "text-top"),
        ("align-top", "vertical-align", "top"),
        ("appearance-auto", "appearance", "auto"),
        ("appearance-none", "appearance", "none"),
        ("auto-cols-auto", "grid-auto-columns", "auto"),
        ("auto-cols-fr", "grid-auto-columns", "minmax(0, 1fr)"),
        ("auto-cols-max", "grid-auto-columns", "max-content"),
        ("auto-cols-min", "grid-auto-columns", "min-content"),
        ("auto-rows-auto", "grid-auto-rows", "auto"),
        ("auto-rows-fr", "grid-auto-rows", "minmax(0, 1fr)"),
        ("auto-rows-max", "grid-auto-rows", "max-content"),
        ("auto-rows-min", "grid-auto-rows", "min-content"),
        ("bg-auto", "background-size", "auto"),
        ("bg-bottom", "background-position", "bottom"),
        ("bg-bottom-left", "background-position", "left bottom"),
        ("bg-bottom-right", "background-position", "right bottom"),
        ("bg-center", "background-position", "center"),
        ("bg-clip-border", "background-clip", "border-box"),
        ("bg-clip-content", "background-clip", "content-box"),
        ("bg-clip-padding", "background-clip", "padding-box"),
        ("bg-clip-text", "background-clip", "text"),
        ("bg-contain", "background-size", "contain"),
        ("bg-cover", "background-size", "cover"),
        ("bg-fixed", "background-attachment", "fixed"),
        ("bg-left", "background-position", "left"),
        ("bg-local", "background-attachment", "local"),
        ("bg-no-repeat", "background-repeat", "no-repeat"),
        ("bg-origin-border", "background-origin", "border-box"),
        ("bg-origin-content", "background-origin", "content-box"),
        ("bg-origin-padding", "background-origin", "padding-box"),
        ("bg-repeat", "background-repeat", "repeat"),
        ("bg-repeat-round", "background-repeat", "round"),
        ("bg-repeat-space", "background-repeat", "space"),
        ("bg-repeat-x", "background-repeat", "repeat-x"),
        ("bg-repeat-y", "background-repeat", "repeat-y"),
        ("bg-right", "background-position", "right"),
        ("bg-scroll", "background-attachment", "scroll"),
        ("bg-top", "background-position", "top"),
        ("bg-top-left", "background-position", "left top"),
        ("bg-top-right", "background-position", "right top"),
        ("border-collapse", "border-collapse", "collapse"),
        ("border-separate", "border-collapse", "separate"),
        ("break-after-all", "break-after", "all"),
        ("break-after-auto", "break-after", "auto"),
        ("break-after-avoid", "break-after", "avoid"),
        ("break-after-avoid-page", "break-after", "avoid-page"),
        ("break-after-column", "break-after", "column"),
        ("break-after-left", "break-after", "left"),
        ("break-after-page", "break-after", "page"),
        ("break-after-right", "break-after", "right"),
        ("break-all", "word-break", "break-all"),
        ("break-before-all", "break-before", "all"),
        ("break-before-auto", "break-before", "auto"),
        ("break-before-avoid", "break-before", "avoid"),
        ("break-before-avoid-page", "break-before", "avoid-page"),
        ("break-before-column", "break-before", "column"),
        ("break-before-left", "break-before", "left"),
        ("break-before-page", "break-before", "page"),
        ("break-before-right", "break-before", "right"),
        ("break-inside-auto", "break-inside", "auto"),
        ("break-inside-avoid", "break-inside", "avoid"),
        ("break-inside-avoid-column", "break-inside", "avoid-column"),
        ("break-inside-avoid-page", "break-inside", "avoid-page"),
        ("break-keep", "word-break", "keep-all"),
        ("caption-bottom", "caption-side", "bottom"),
        ("caption-top", "caption-side", "top"),
        ("clear-both", "clear", "both"),
        ("clear-end", "clear", "inline-end"),
        ("clear-left", "clear", "left"),
        ("clear-none", "clear", "none"),
        ("clear-right", "clear", "right"),
        ("clear-start", "clear", "inline-start"),
        ("contain-content", "contain", "content"),
        ("contain-inline-size", "contain", "inline-size"),
        ("contain-layout", "contain", "layout"),
        ("contain-none", "contain", "none"),
        ("contain-paint", "contain", "paint"),
        ("contain-size", "contain", "size"),
        ("contain-strict", "contain", "strict"),
        ("contain-style", "contain", "style"),
        ("delay-75", "transition-delay", "75ms"),
        ("delay-100", "transition-delay", "100ms"),
        ("delay-150", "transition-delay", "150ms"),
        ("delay-200", "transition-delay", "200ms"),
        ("delay-300", "transition-delay", "300ms"),
        ("delay-500", "transition-delay", "500ms"),
        ("delay-700", "transition-delay", "700ms"),
        ("delay-1000", "transition-delay", "1000ms"),
        ("diagonal-fractions", "font-variant-numeric", "diagonal-fractions"),
        ("field-sizing-content", "field-sizing", "content"),
        ("field-sizing-fixed", "field-sizing", "fixed"),
        ("float-end", "float", "inline-end"),
        ("float-left", "float", "left"),
        ("float-none", "float", "none"),
        ("float-right", "float", "right"),
        ("float-start", "float", "inline-start"),
        // The percentage forms, spelled out rather than parsed: Tailwind
        // defines exactly these ten, and accepting any number would
        // compile `font-stretch-63%`, which it does not.
        ("font-stretch-50%", "font-stretch", "50%"),
        ("font-stretch-75%", "font-stretch", "75%"),
        ("font-stretch-90%", "font-stretch", "90%"),
        ("font-stretch-95%", "font-stretch", "95%"),
        ("font-stretch-100%", "font-stretch", "100%"),
        ("font-stretch-105%", "font-stretch", "105%"),
        ("font-stretch-110%", "font-stretch", "110%"),
        ("font-stretch-125%", "font-stretch", "125%"),
        ("font-stretch-150%", "font-stretch", "150%"),
        ("font-stretch-200%", "font-stretch", "200%"),
        ("font-stretch-condensed", "font-stretch", "condensed"),
        ("font-stretch-expanded", "font-stretch", "expanded"),
        ("font-stretch-extra-condensed", "font-stretch", "extra-condensed"),
        ("font-stretch-extra-expanded", "font-stretch", "extra-expanded"),
        ("font-stretch-normal", "font-stretch", "normal"),
        ("font-stretch-semi-condensed", "font-stretch", "semi-condensed"),
        ("font-stretch-semi-expanded", "font-stretch", "semi-expanded"),
        ("font-stretch-ultra-condensed", "font-stretch", "ultra-condensed"),
        ("font-stretch-ultra-expanded", "font-stretch", "ultra-expanded"),
        ("forced-color-adjust-auto", "forced-color-adjust", "auto"),
        ("forced-color-adjust-none", "forced-color-adjust", "none"),
        ("grid-flow-col", "grid-auto-flow", "column"),
        ("grid-flow-col-dense", "grid-auto-flow", "column dense"),
        ("grid-flow-dense", "grid-auto-flow", "dense"),
        ("grid-flow-row", "grid-auto-flow", "row"),
        ("grid-flow-row-dense", "grid-auto-flow", "row dense"),
        ("justify-items-center", "justify-items", "center"),
        ("justify-items-center-safe", "justify-items", "safe center"),
        ("justify-items-end", "justify-items", "end"),
        ("justify-items-end-safe", "justify-items", "safe end"),
        ("justify-items-normal", "justify-items", "normal"),
        ("justify-items-start", "justify-items", "start"),
        ("justify-items-stretch", "justify-items", "stretch"),
        ("justify-self-auto", "justify-self", "auto"),
        ("justify-self-center", "justify-self", "center"),
        ("justify-self-center-safe", "justify-self", "safe center"),
        ("justify-self-end", "justify-self", "flex-end"),
        ("justify-self-end-safe", "justify-self", "safe flex-end"),
        ("justify-self-start", "justify-self", "flex-start"),
        ("justify-self-stretch", "justify-self", "stretch"),
        ("lining-nums", "font-variant-numeric", "lining-nums"),
        ("list-decimal", "list-style-type", "decimal"),
        ("list-disc", "list-style-type", "disc"),
        ("list-image-none", "list-style-image", "none"),
        ("list-inside", "list-style-position", "inside"),
        ("list-none", "list-style-type", "none"),
        ("list-outside", "list-style-position", "outside"),
        ("normal-nums", "font-variant-numeric", "normal"),
        ("object-bottom", "object-position", "bottom"),
        ("object-bottom-left", "object-position", "left bottom"),
        ("object-bottom-right", "object-position", "right bottom"),
        ("object-center", "object-position", "center"),
        ("object-left", "object-position", "left"),
        ("object-right", "object-position", "right"),
        ("object-top", "object-position", "top"),
        ("object-top-left", "object-position", "left top"),
        ("object-top-right", "object-position", "right top"),
        ("oldstyle-nums", "font-variant-numeric", "oldstyle-nums"),
        ("ordinal", "font-variant-numeric", "ordinal"),
        ("origin-bottom", "transform-origin", "bottom"),
        ("origin-bottom-left", "transform-origin", "0 100%"),
        ("origin-bottom-right", "transform-origin", "100% 100%"),
        ("origin-center", "transform-origin", "center"),
        ("origin-left", "transform-origin", "0"),
        ("origin-right", "transform-origin", "100%"),
        ("origin-top", "transform-origin", "top"),
        ("origin-top-left", "transform-origin", "0 0"),
        ("origin-top-right", "transform-origin", "100% 0"),
        ("overscroll-auto", "overscroll-behavior", "auto"),
        ("overscroll-contain", "overscroll-behavior", "contain"),
        ("overscroll-none", "overscroll-behavior", "none"),
        ("overscroll-x-auto", "overscroll-behavior-x", "auto"),
        ("overscroll-x-contain", "overscroll-behavior-x", "contain"),
        ("overscroll-x-none", "overscroll-behavior-x", "none"),
        ("overscroll-y-auto", "overscroll-behavior-y", "auto"),
        ("overscroll-y-contain", "overscroll-behavior-y", "contain"),
        ("overscroll-y-none", "overscroll-behavior-y", "none"),
        ("perspective-distant", "perspective", "1200px"),
        ("perspective-dramatic", "perspective", "100px"),
        ("perspective-midrange", "perspective", "800px"),
        ("perspective-near", "perspective", "300px"),
        ("perspective-none", "perspective", "none"),
        ("perspective-normal", "perspective", "500px"),
        ("perspective-origin-bottom", "perspective-origin", "bottom"),
        ("perspective-origin-bottom-left", "perspective-origin", "0 100%"),
        ("perspective-origin-bottom-right", "perspective-origin", "100% 100%"),
        ("perspective-origin-center", "perspective-origin", "center"),
        ("perspective-origin-left", "perspective-origin", "0"),
        ("perspective-origin-right", "perspective-origin", "100%"),
        ("perspective-origin-top", "perspective-origin", "top"),
        ("perspective-origin-top-left", "perspective-origin", "0 0"),
        ("perspective-origin-top-right", "perspective-origin", "100% 0"),
        ("place-content-around", "place-content", "space-around"),
        ("place-content-baseline", "place-content", "baseline"),
        ("place-content-between", "place-content", "space-between"),
        ("place-content-center", "place-content", "center"),
        ("place-content-center-safe", "place-content", "safe center"),
        ("place-content-end", "place-content", "end"),
        ("place-content-end-safe", "place-content", "safe end"),
        ("place-content-evenly", "place-content", "space-evenly"),
        ("place-content-start", "place-content", "start"),
        ("place-content-stretch", "place-content", "stretch"),
        ("place-items-baseline", "place-items", "baseline"),
        ("place-items-center", "place-items", "center"),
        ("place-items-center-safe", "place-items", "safe center"),
        ("place-items-end", "place-items", "end"),
        ("place-items-end-safe", "place-items", "safe end"),
        ("place-items-start", "place-items", "start"),
        ("place-items-stretch", "place-items", "stretch"),
        ("place-self-auto", "place-self", "auto"),
        ("place-self-center", "place-self", "center"),
        ("place-self-center-safe", "place-self", "safe center"),
        ("place-self-end", "place-self", "end"),
        ("place-self-end-safe", "place-self", "safe end"),
        ("place-self-start", "place-self", "start"),
        ("place-self-stretch", "place-self", "stretch"),
        ("proportional-nums", "font-variant-numeric", "proportional-nums"),
        ("resize", "resize", "both"),
        ("resize-none", "resize", "none"),
        ("resize-x", "resize", "horizontal"),
        ("resize-y", "resize", "vertical"),
        ("scheme-dark", "color-scheme", "dark"),
        ("scheme-light", "color-scheme", "light"),
        ("scheme-light-dark", "color-scheme", "light dark"),
        ("scheme-normal", "color-scheme", "normal"),
        ("scheme-only-dark", "color-scheme", "only dark"),
        ("scheme-only-light", "color-scheme", "only light"),
        ("slashed-zero", "font-variant-numeric", "slashed-zero"),
        ("snap-align-none", "scroll-snap-align", "none"),
        ("snap-always", "scroll-snap-stop", "always"),
        ("snap-both", "scroll-snap-type", "both proximity"),
        ("snap-center", "scroll-snap-align", "center"),
        ("snap-end", "scroll-snap-align", "end"),
        ("snap-none", "scroll-snap-type", "none"),
        ("snap-normal", "scroll-snap-stop", "normal"),
        ("snap-start", "scroll-snap-align", "start"),
        ("snap-x", "scroll-snap-type", "x proximity"),
        ("snap-y", "scroll-snap-type", "y proximity"),
        ("stacked-fractions", "font-variant-numeric", "stacked-fractions"),
        ("tab-2", "tab-size", "2"),
        ("tab-4", "tab-size", "4"),
        ("tab-8", "tab-size", "8"),
        ("table-auto", "table-layout", "auto"),
        ("table-fixed", "table-layout", "fixed"),
        ("tabular-nums", "font-variant-numeric", "tabular-nums"),
        ("text-balance", "text-wrap", "balance"),
        ("text-nowrap", "text-wrap", "nowrap"),
        ("text-pretty", "text-wrap", "pretty"),
        ("text-shadow-2xs", "text-shadow", "0 1px 0 rgb(0 0 0 / 0.15)"),
        ("text-shadow-lg", "text-shadow", "0 1px 2px rgb(0 0 0 / 0.1), 0 3px 2px rgb(0 0 0 / 0.1), 0 4px 8px rgb(0 0 0 / 0.1)"),
        ("text-shadow-md", "text-shadow", "0 1px 1px rgb(0 0 0 / 0.1), 0 1px 2px rgb(0 0 0 / 0.1), 0 2px 4px rgb(0 0 0 / 0.1)"),
        ("text-shadow-none", "text-shadow", "none"),
        ("text-shadow-sm", "text-shadow", "0 1px 0 rgb(0 0 0 / 0.075), 0 1px 1px rgb(0 0 0 / 0.075), 0 2px 2px rgb(0 0 0 / 0.075)"),
        ("text-shadow-xs", "text-shadow", "0 1px 1px rgb(0 0 0 / 0.2)"),
        ("text-wrap", "text-wrap", "wrap"),
        ("touch-auto", "touch-action", "auto"),
        ("touch-manipulation", "touch-action", "manipulation"),
        ("touch-none", "touch-action", "none"),
        ("touch-pan-down", "touch-action", "pan-down"),
        ("touch-pan-left", "touch-action", "pan-left"),
        ("touch-pan-right", "touch-action", "pan-right"),
        ("touch-pan-up", "touch-action", "pan-up"),
        ("touch-pan-x", "touch-action", "pan-x"),
        ("touch-pan-y", "touch-action", "pan-y"),
        ("touch-pinch-zoom", "touch-action", "pinch-zoom"),
        ("transform-3d", "transform-style", "preserve-3d"),
        ("transform-border", "transform-box", "border-box"),
        ("transform-content", "transform-box", "content-box"),
        ("transform-fill", "transform-box", "fill-box"),
        ("transform-flat", "transform-style", "flat"),
        ("transform-stroke", "transform-box", "stroke-box"),
        ("transform-view", "transform-box", "view-box"),
        ("transition-discrete", "transition-behavior", "allow-discrete"),
        ("transition-normal", "transition-behavior", "normal"),
        ("will-change-auto", "will-change", "auto"),
        ("will-change-contents", "will-change", "contents"),
        ("will-change-scroll", "will-change", "scroll-position"),
        ("will-change-transform", "will-change", "transform"),
        ("wrap-anywhere", "overflow-wrap", "anywhere"),
        ("wrap-break-word", "overflow-wrap", "break-word"),
        ("wrap-normal", "overflow-wrap", "normal"),
        ("zoom-50", "zoom", "50%"),
        ("zoom-75", "zoom", "75%"),
        ("zoom-90", "zoom", "90%"),
        ("zoom-95", "zoom", "95%"),
        ("zoom-100", "zoom", "100%"),
        ("zoom-105", "zoom", "105%"),
        ("zoom-110", "zoom", "110%"),
        ("zoom-125", "zoom", "125%"),
        ("zoom-150", "zoom", "150%"),
        ("zoom-200", "zoom", "200%"),
];

/// `sr-only` / `not-sr-only`: visible to a screen reader, not to the eye.
///
/// Expanded into the properties Hozo already models rather than held as a
/// blob, so that a `w-4` written beside it still overrides the width the
/// way it does on Web -- `dedupe_key` can only do that if each declaration
/// arrives as its own property.
///
/// The mechanism is a one-pixel clipped box rather than `display: none` or
/// `visibility: hidden`, and that distinction is the whole point: those two
/// remove the element from the accessibility tree, which is the opposite of
/// what this is for.
fn parse_screen_reader_only(token: &str) -> Option<Vec<StyleProperty>> {
    let visually_hidden = token == "sr-only";
    if !visually_hidden && token != "not-sr-only" {
        return None;
    }
    let size = |px: f64| {
        if visually_hidden {
            Dimension::Length(Length::Px(px))
        } else {
            Dimension::Auto
        }
    };
    let margin = Dimension::Length(Length::Px(if visually_hidden { -1.0 } else { 0.0 }));
    let mut props = vec![
        StyleProperty::Position(if visually_hidden { Position::Absolute } else { Position::Static }),
        StyleProperty::Width(size(1.0)),
        StyleProperty::Height(size(1.0)),
        StyleProperty::PaddingTop(Length::Px(0.0)),
        StyleProperty::PaddingRight(Length::Px(0.0)),
        StyleProperty::PaddingBottom(Length::Px(0.0)),
        StyleProperty::PaddingLeft(Length::Px(0.0)),
        StyleProperty::MarginTop(margin.clone()),
        StyleProperty::MarginRight(margin.clone()),
        StyleProperty::MarginBottom(margin.clone()),
        StyleProperty::MarginLeft(margin),
        StyleProperty::Overflow(if visually_hidden { Overflow::Hidden } else { Overflow::Visible }),
        StyleProperty::Keyword("clip-path", if visually_hidden { "inset(50%)" } else { "none" }),
        StyleProperty::WhiteSpace(if visually_hidden {
            WhiteSpace::NoWrap
        } else {
            WhiteSpace::Normal
        }),
    ];
    // Only `sr-only` zeroes the border; `not-sr-only` leaves whatever the
    // element had, which is why this isn't symmetric.
    //
    // Widths only, unlike `all_sides_border`: that writes a `solid` style
    // alongside, which exists so a width has something to render as. A zero
    // width has nothing to render either way, and Tailwind writes no style
    // here.
    if visually_hidden {
        props.extend([
            StyleProperty::BorderTopWidth(Length::Px(0.0)),
            StyleProperty::BorderRightWidth(Length::Px(0.0)),
            StyleProperty::BorderBottomWidth(Length::Px(0.0)),
            StyleProperty::BorderLeftWidth(Length::Px(0.0)),
        ]);
    }
    Some(props)
}

/// The utilities that are two declarations of the same value.
///
/// Almost all of them are a vendor prefix beside the standard property,
/// which Tailwind still emits because Safari and Firefox need them here.
/// `break-normal` is the odd one: two genuinely different properties that
/// Tailwind sets together because "don't break words" needs both.
fn keyword_pair_utility(token: &str) -> Option<StyleProperty> {
    const PAIRS: &[(&str, &str, &str, &str, &str)] = &[
        ("hyphens-auto", "-webkit-hyphens", "auto", "hyphens", "auto"),
        ("hyphens-manual", "-webkit-hyphens", "manual", "hyphens", "manual"),
        ("hyphens-none", "-webkit-hyphens", "none", "hyphens", "none"),
        (
            "antialiased",
            "-webkit-font-smoothing",
            "antialiased",
            "-moz-osx-font-smoothing",
            "grayscale",
        ),
        (
            "subpixel-antialiased",
            "-webkit-font-smoothing",
            "auto",
            "-moz-osx-font-smoothing",
            "auto",
        ),
        (
            "box-decoration-clone",
            "-webkit-box-decoration-break",
            "clone",
            "box-decoration-break",
            "clone",
        ),
        (
            "box-decoration-slice",
            "-webkit-box-decoration-break",
            "slice",
            "box-decoration-break",
            "slice",
        ),
        ("break-normal", "overflow-wrap", "normal", "word-break", "normal"),
    ];
    PAIRS.iter().find(|(name, ..)| *name == token).map(|(_, p1, v1, p2, v2)| {
        StyleProperty::KeywordPair(p1, v1, p2, v2)
    })
}

/// The one-declaration utilities; see `KEYWORD_UTILITIES`.
fn keyword_utility(token: &str) -> Option<StyleProperty> {
    KEYWORD_UTILITIES
        .iter()
        .find(|(name, _, _)| *name == token)
        .map(|(_, property, value)| StyleProperty::Keyword(property, value))
}

/// The families that are a fixed list of CSS keywords and nothing else:
/// `flex-<n>`, the two blend modes, and the display keywords beyond the
/// three Yoga implements.
fn parse_keyword_utility(token: &str) -> Option<StyleProperty> {
    if let Some((n, d)) = token.strip_prefix("flex-").and_then(|r| r.split_once('/')) {
        if let (Ok(n), Ok(d)) = (n.parse::<u32>(), d.parse::<u32>()) {
            return Some(StyleProperty::Flex(FlexShorthand::Fraction(n, d)));
        }
    }
    // `flex-<n>` is `n 1 0%`, i.e. grow by n and start from nothing --
    // which is `FlexShorthand::Grow`, the same shape `flex-1` already had.
    if let Some(rest) = token.strip_prefix("flex-") {
        if let Ok(grow) = rest.parse::<f64>() {
            return Some(StyleProperty::Flex(FlexShorthand::Grow(grow)));
        }
    }
    /// CSS's blend modes. Shared by `mix-blend-*` and `bg-blend-*`, which
    /// take the same list applied to different things.
    const BLEND_MODES: &[&str] = &[
        "normal", "multiply", "screen", "overlay", "darken", "lighten", "color-dodge",
        "color-burn", "hard-light", "soft-light", "difference", "exclusion", "hue", "saturation",
        "color", "luminosity", "plus-darker", "plus-lighter",
    ];
    if let Some(rest) = token.strip_prefix("mix-blend-") {
        return blend_mode(rest, BLEND_MODES).map(StyleProperty::MixBlendMode);
    }
    if let Some(rest) = token.strip_prefix("bg-blend-") {
        return blend_mode(rest, BLEND_MODES).map(StyleProperty::BackgroundBlendMode);
    }

    // Display keywords with backend-specific handling. `inline-flex` and
    // `block` are typed because Native lowers them to constrained forms of
    // Yoga flex; `grid` is typed so Native can refuse it by name.
    const DISPLAYS: &[&str] = &[
        "inline", "inline-block", "inline-grid", "inline-table", "flow-root", "list-item",
        "table", "table-caption", "table-cell", "table-column", "table-column-group",
        "table-footer-group", "table-header-group", "table-row", "table-row-group", "hidden",
    ];
    if let Some(keyword) = DISPLAYS.iter().find(|k| **k == token) {
        // `hidden` is Tailwind's name for `display: none`, which Yoga does
        // have -- so it is not one of the grouped keywords.
        if *keyword == "hidden" {
            return Some(StyleProperty::Display(Display::None));
        }
        return Some(StyleProperty::Display(Display::Css(keyword)));
    }
    None
}

fn blend_mode(suffix: &str, modes: &'static [&'static str]) -> Option<&'static str> {
    modes.iter().copied().find(|m| *m == suffix)
}

/// The sizes CSS states as a keyword or a unit Hozo can't resolve:
/// intrinsic sizing, the chrome-aware viewport units, and `lh`. Shared by
/// every size family, since Tailwind offers them on all of them.
fn parse_css_size_suffix(suffix: &str) -> Option<Dimension> {
    Some(Dimension::Css(String::from(match suffix {
        "fit" => "fit-content",
        "max" => "max-content",
        "min" => "min-content",
        "dvh" => "100dvh",
        "dvw" => "100dvw",
        "lvh" => "100lvh",
        "lvw" => "100lvw",
        "svh" => "100svh",
        "svw" => "100svw",
        "lh" => "1lh",
        _ => return None,
    })))
}

/// The values only the *max* and *min* size families take.
///
/// Kept out of the shared suffix table because they are not dimensions in
/// general: putting `none` there made `translate-none` parse as a
/// translation of "none none" rather than as switching the property off.
fn parse_extremum_suffix(suffix: &str) -> Option<Dimension> {
    Some(Dimension::Css(String::from(match suffix {
        "none" => "none",
        // Tailwind's measure-of-text width, and the one place `ch` appears.
        "prose" => "65ch",
        _ => return None,
    })))
}

fn parse_inline_size_suffix(suffix: &str) -> Option<Dimension> {
    let rem = match suffix {
        "3xs" => 16.0,
        "2xs" => 18.0,
        "xs" => 20.0,
        "sm" => 24.0,
        "md" => 28.0,
        "lg" => 32.0,
        "xl" => 36.0,
        "2xl" => 42.0,
        "3xl" => 48.0,
        "4xl" => 56.0,
        "5xl" => 64.0,
        "6xl" => 72.0,
        "7xl" => 80.0,
        _ => return parse_css_size_suffix(suffix).or_else(|| parse_dimension_suffix(suffix)),
    };
    Some(Dimension::Length(Length::Px(rem * 16.0)))
}

/// Width/height accept more than the spacing scale: `w-1/2` fractions and
/// `w-full`/`w-auto` keywords (the latter handled by the exact-match table).
fn parse_dimension_suffix(suffix: &str) -> Option<Dimension> {
    if let Some(css) = parse_css_size_suffix(suffix) {
        return Some(css);
    }
    match suffix {
        "auto" => return Some(Dimension::Auto),
        "full" => return Some(Dimension::Percent(100.0)),
        _ => {}
    }
    if let Some((num, denom)) = suffix.split_once('/') {
        let num: f64 = num.parse().ok()?;
        let denom: f64 = denom.parse().ok()?;
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
        // Direction-relative: `ps`/`pe` and `ms`/`me` are Tailwind's
        // logical counterparts to `pl`/`pr` and `ml`/`mr`.
        "ps" => Some(StyleProperty::PaddingInlineStart(value)),
        "pe" => Some(StyleProperty::PaddingInlineEnd(value)),
        // Margins accept `auto` where padding doesn't, so they take the
        // wider `Dimension` and are parsed separately below.
        "mt" | "mr" | "mb" | "ml" | "ms" | "me" => None,
        "start" => Some(StyleProperty::InsetInlineStart(Dimension::Length(value))),
        "end" => Some(StyleProperty::InsetInlineEnd(Dimension::Length(value))),
        _ => None,
    }
}

/// Strips every recognized `variant:` prefix, innermost last:
/// `md:hover:flex` -> `(All([Responsive(Md), Hover]), "flex")`.
///
/// The loop is the whole point. Stripping one level and handing the rest
/// to the utility parser meant `md:hover:bg-blue-500` compiled to nothing
/// at all, silently -- `hover:bg-blue-500` is not a utility name, so
/// nothing matched and nothing complained.
///
/// Stops at the first prefix it doesn't recognise rather than skipping it,
/// because the remainder may legitimately contain a colon: an arbitrary
/// value can (`bg-[url(https://x)]`), and an unrecognised variant is a
/// gap that should stay visible rather than being silently applied
/// unconditionally.
pub fn parse_variant_prefix(token: &str) -> (Condition, &str) {
    let mut conditions: Vec<Condition> = Vec::new();
    let mut rest = token;
    loop {
        let (condition, tail) = parse_one_variant(rest);
        if condition == Condition::Always {
            break;
        }
        conditions.push(condition);
        rest = tail;
    }
    let condition = match conditions.len() {
        0 => Condition::Always,
        // Not wrapped, so a single variant stays the value it always was
        // and everything that matches on `Condition::Hover` keeps working.
        1 => conditions.pop().expect("length checked"),
        _ => Condition::All(conditions),
    };
    (condition, rest)
}

/// Strips one recognized `variant:` prefix, or returns `Always` and the
/// token unchanged.
fn parse_one_variant(token: &str) -> (Condition, &str) {
    // Before the named variants, because an arbitrary one can *contain*
    // them: `[&:hover]:p-4` starts with a bracket and would otherwise be
    // cut at the wrong colon.
    if let Some((selector, rest)) = crate::arbitrary::split_variant(token) {
        // An at-rule and a selector wrap a rule differently -- one around
        // it, one through its head -- so which it is has to be decided
        // here, where the text is, rather than by looking for a leading
        // `@` at emit time.
        let condition = if selector.starts_with('@') {
            Condition::ArbitraryAtRule(selector)
        } else {
            Condition::ArbitrarySelector(selector)
        };
        return (condition, rest);
    }
    // `group-` and `peer-` wrap whatever variant follows them, so they
    // are parsed by recursion rather than by a list of the combinations.
    // An inner variant Hozo does not know leaves `Always`, and the token
    // falls through to the unsupported-variant diagnostic whole.
    for (prefix, wrap) in [
        ("group-", true),
        ("peer-", false),
    ] {
        if let Some(inner) = token.strip_prefix(prefix) {
            let (condition, tail) = parse_one_variant(inner);
            if condition.is_elemental() {
                return (
                    if wrap {
                        Condition::Group(Box::new(condition))
                    } else {
                        Condition::Peer(Box::new(condition))
                    },
                    tail,
                );
            }
        }
    }
    if let Some(rest) = token.strip_prefix("hover:") {
        return (Condition::Hover, rest);
    }
    if let Some(rest) = token.strip_prefix("focus:") {
        return (Condition::Focus, rest);
    }
    if let Some(rest) = token.strip_prefix("disabled:") {
        return (Condition::Disabled, rest);
    }
    if let Some(rest) = token.strip_prefix("enabled:") {
        return (Condition::Enabled, rest);
    }
    if let Some(rest) = token.strip_prefix("focus-visible:") {
        return (Condition::FocusVisible, rest);
    }
    // `aria-checked:`, `aria-expanded:` and the rest. The list of states
    // comes from Tailwind rather than from a set of names that looked
    // complete -- every one is an ARIA attribute with a `"true"`/`"false"`
    // value, which is why there is a shortlist at all: `aria-sort` takes
    // four words and is written `aria-[sort=ascending]:` instead.
    if let Some(rest) = token.strip_prefix("aria-") {
        if let Some((state, tail)) = rest.split_once(':') {
            if crate::tailwind_variants::ARIA_VARIANT_STATES.contains(&state) {
                return (Condition::Aria(state.to_string()), tail);
            }
        }
    }
    // `pressed:` is Hozo's own name, kept because it is the one that
    // reads correctly on a device. `active:` is Tailwind's for the same
    // state, and refusing it would mean a Tailwind class that compiles
    // everywhere except here.
    if let Some(rest) = token.strip_prefix("pressed:").or_else(|| token.strip_prefix("active:")) {
        return (Condition::Pressed, rest);
    }
    if let Some(rest) = token.strip_prefix("dark:") {
        return (Condition::Dark, rest);
    }
    if let Some(rest) = token.strip_prefix("first:") {
        return (Condition::FirstChild, rest);
    }
    if let Some(rest) = token.strip_prefix("last:") {
        return (Condition::LastChild, rest);
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
    (condition, expand_negatable(base))
}

/// Every condition/properties group one class token produces.
///
/// Almost always exactly one, which is why `expand_utility` returns a
/// single pair and everything is written against it. `container` is the
/// exception: it is `width: 100%` plus a max-width at each breakpoint, so
/// one token writes six conditions.
///
/// Handled by expanding it into the tokens it stands for rather than by
/// teaching the parser to return several conditions. That keeps one code
/// path -- each piece is parsed exactly as if the author had written it --
/// and it makes `md:container` fall out for free, since the variant
/// prefix rides along and `Condition::All` nests the two width queries the
/// way Tailwind does.
pub fn expand_class(token: &str) -> Vec<(Condition, Vec<StyleProperty>)> {
    match expand_shorthand(token) {
        Some(tokens) => tokens.iter().map(|token| expand_utility(token)).collect(),
        None => vec![expand_utility(token)],
    }
}

/// The tokens a shorthand utility stands for, with its own variant prefix
/// carried onto each.
///
/// The max-widths are literal `rem` values rather than the `max-w-*` scale,
/// which is a different set of numbers: `max-w-sm` is 24rem and the `sm`
/// container is 40rem. They track the *breakpoints*, and Hozo's
/// breakpoints are still fixed -- when they become theme values these
/// should follow them there.
fn expand_shorthand(token: &str) -> Option<Vec<String>> {
    const CONTAINER: &[&str] = &[
        "w-full",
        "sm:max-w-[40rem]",
        "md:max-w-[48rem]",
        "lg:max-w-[64rem]",
        "xl:max-w-[80rem]",
        "2xl:max-w-[96rem]",
    ];
    let (_, base) = parse_variant_prefix(token);
    if base != "container" {
        return None;
    }
    let prefix = &token[..token.len() - base.len()];
    Some(CONTAINER.iter().map(|part| format!("{prefix}{part}")).collect())
}

/// Handles Tailwind's leading `-` once, for every family, by expanding the
/// positive form and flipping the result.
///
/// Doing it here rather than in each parser is what keeps `-mt-4`, `-top-4`
/// and `-translate-x-1/2` from each needing their own sign handling -- the
/// families that take a negative are exactly the ones whose properties are
/// numeric, and `negated` decides that per property rather than per parser.
fn expand_negatable(token: &str) -> Vec<StyleProperty> {
    match token.strip_prefix('-') {
        // A leading `-` on something that doesn't take one (`-p-4`) yields
        // nothing rather than an invented negative padding.
        Some(positive) => {
            expand_base_utility(positive).into_iter().map(negated).collect::<Option<Vec<_>>>()
        }
        None => Some(expand_base_utility(token)),
    }
    .unwrap_or_default()
}

/// Flips one property's sign, or `None` if it has no meaningful negative.
///
/// Tailwind only generates the `-` form where CSS accepts a negative value,
/// so refusing here keeps Hozo from accepting more than Tailwind does.
/// Flips a length's sign, keeping it in whatever unit it was written in.
///
/// A spacing step negates as a step: `-mt-4` is minus four steps, and
/// resolving it to pixels first would freeze the project's scale into the
/// sign flip. Without an arm for it the whole utility was dropped --
/// negation returned `None`, and the caller reads that as "this property
/// has no negative form".
/// Flips an angle's sign.
///
/// `None` for an angle carried as text: `-rotate-x-[1.5em]` would have to
/// negate a string, and prefixing a `-` would produce `--1.5em` for an
/// author who already wrote one. Tailwind refuses the same combination.
fn negated_angle(angle: Angle) -> Option<Angle> {
    match angle {
        Angle::Deg(degrees) => Some(Angle::Deg(signed(degrees, true))),
        Angle::Css(_) => None,
    }
}

/// Flips a scale's sign. `None` for the text case, for the same reason
/// `negated_angle` gives.
fn negated_scale(scale: Scale) -> Option<Scale> {
    match scale {
        Scale::Percent(value) => Some(Scale::Percent(signed(value, true))),
        Scale::Css(_) => None,
    }
}

fn negated_length(length: Length) -> Option<Length> {
    Some(match length {
        Length::Px(v) => Length::Px(signed(v, true)),
        Length::Spacing(v) => Length::Spacing(signed(v, true)),
        // Negated in place, keeping the unit: `-mt-[2rem]` is `-2rem`, and
        // the unit is what makes it a length at all.
        Length::Unit(v, unit) => Length::Unit(signed(v, true), unit),
    })
}

fn negated(prop: StyleProperty) -> Option<StyleProperty> {
    fn flip(d: Dimension) -> Option<Dimension> {
        Some(match d {
            Dimension::Length(l) => Dimension::Length(negated_length(l)?),
            Dimension::Percent(v) => Dimension::Percent(signed(v, true)),
            // `auto` and the viewport units have no negative form.
            _ => return None,
        })
    }
    Some(match prop {
        StyleProperty::MarginTop(d) => StyleProperty::MarginTop(flip(d)?),
        StyleProperty::MarginRight(d) => StyleProperty::MarginRight(flip(d)?),
        StyleProperty::MarginBottom(d) => StyleProperty::MarginBottom(flip(d)?),
        StyleProperty::MarginLeft(d) => StyleProperty::MarginLeft(flip(d)?),
        StyleProperty::MarginInlineStart(d) => StyleProperty::MarginInlineStart(flip(d)?),
        StyleProperty::MarginInlineEnd(d) => StyleProperty::MarginInlineEnd(flip(d)?),
        StyleProperty::MarginBlockStart(d) => StyleProperty::MarginBlockStart(flip(d)?),
        StyleProperty::MarginBlockEnd(d) => StyleProperty::MarginBlockEnd(flip(d)?),
        StyleProperty::TextIndent(d) => StyleProperty::TextIndent(flip(d)?),
        // Only the em form negates: an absolute tracking is already folded
        // against a font size, and Tailwind has no negative form of it.
        StyleProperty::LetterSpacing(LetterSpacing::Em(Em(v))) => {
            StyleProperty::LetterSpacing(LetterSpacing::Em(Em(signed(v, true))))
        }
        StyleProperty::TextUnderlineOffset(d) => StyleProperty::TextUnderlineOffset(flip(d)?),
        StyleProperty::InsetTop(d) => StyleProperty::InsetTop(flip(d)?),
        StyleProperty::InsetRight(d) => StyleProperty::InsetRight(flip(d)?),
        StyleProperty::InsetBottom(d) => StyleProperty::InsetBottom(flip(d)?),
        StyleProperty::InsetLeft(d) => StyleProperty::InsetLeft(flip(d)?),
        StyleProperty::InsetInlineStart(d) => StyleProperty::InsetInlineStart(flip(d)?),
        StyleProperty::InsetInlineEnd(d) => StyleProperty::InsetInlineEnd(flip(d)?),
        StyleProperty::InsetInline(d) => StyleProperty::InsetInline(flip(d)?),
        StyleProperty::InsetBlock(d) => StyleProperty::InsetBlock(flip(d)?),
        StyleProperty::InsetBlockStart(d) => StyleProperty::InsetBlockStart(flip(d)?),
        StyleProperty::InsetBlockEnd(d) => StyleProperty::InsetBlockEnd(flip(d)?),
        StyleProperty::TranslateX(d) => StyleProperty::TranslateX(flip(d)?),
        StyleProperty::TranslateY(d) => StyleProperty::TranslateY(flip(d)?),
        StyleProperty::TranslateZ(d) => StyleProperty::TranslateZ(flip(d)?),
        StyleProperty::OutlineOffset(l) => StyleProperty::OutlineOffset(negated_length(l)?),
        StyleProperty::ZIndex(Some(z)) => StyleProperty::ZIndex(Some(-z)),
        // A marker rather than a value: negating `scale-z-50` must keep the
        // three-value form, so this negates to itself. Without an arm here
        // `negated` returned None and dropped the whole utility.
        StyleProperty::Scale3d => StyleProperty::Scale3d,
        StyleProperty::Order(n) => StyleProperty::Order(-n),
        // A negative grid line counts back from the end of the explicit
        // grid, so these negate rather than being rejected.
        StyleProperty::GridColumnStart(GridLine::Line(n)) => {
            StyleProperty::GridColumnStart(GridLine::Line(-n))
        }
        StyleProperty::GridColumnEnd(GridLine::Line(n)) => {
            StyleProperty::GridColumnEnd(GridLine::Line(-n))
        }
        StyleProperty::GridRowStart(GridLine::Line(n)) => {
            StyleProperty::GridRowStart(GridLine::Line(-n))
        }
        StyleProperty::GridRowEnd(GridLine::Line(n)) => {
            StyleProperty::GridRowEnd(GridLine::Line(-n))
        }
        StyleProperty::SpaceX(d) => StyleProperty::SpaceX(flip(d)?),
        StyleProperty::SpaceY(d) => StyleProperty::SpaceY(flip(d)?),
        StyleProperty::ScrollMargin(edge, l) => StyleProperty::ScrollMargin(edge, negated_length(l)?),
        StyleProperty::Rotate(a) => StyleProperty::Rotate(negated_angle(a)?),
        StyleProperty::ScaleX(s) => StyleProperty::ScaleX(negated_scale(s)?),
        StyleProperty::ScaleY(s) => StyleProperty::ScaleY(negated_scale(s)?),
        StyleProperty::ScaleZ(s) => StyleProperty::ScaleZ(negated_scale(s)?),
        StyleProperty::RotateX(a) => StyleProperty::RotateX(negated_angle(a)?),
        StyleProperty::RotateY(a) => StyleProperty::RotateY(negated_angle(a)?),
        StyleProperty::RotateZ(a) => StyleProperty::RotateZ(negated_angle(a)?),
        StyleProperty::SkewX(a) => StyleProperty::SkewX(negated_angle(a)?),
        StyleProperty::SkewY(a) => StyleProperty::SkewY(negated_angle(a)?),
        StyleProperty::MaskSlotArgument(slot, a) => StyleProperty::MaskSlotArgument(slot, negated_angle(a)?),
        _ => return None,
    })
}

/// Multi-side utilities (`p-6`, `px-4`, `py-2`, `m-6`, `mx-4`, `my-2`,
/// `gap-x-2`, `gap-y-2`) expand to more than one longhand property, so they
/// can't fit through `parse_utility`'s one-token-to-one-property shape.
fn expand_base_utility(token: &str) -> Vec<StyleProperty> {
    // First, and deliberately so. Every arm below this ends in a colour
    // catch-all that accepts whatever the specific forms declined, so a
    // bracket reaching them came out as a palette token: `text-[14px]`
    // compiled to `color: var(--hozo-color-[14px])`. Ordering is what
    // fixes that -- an arbitrary value is unambiguous, and nothing further
    // down has a better claim on it.
    if crate::arbitrary::is_arbitrary(token) {
        return crate::arbitrary::properties(token).unwrap_or_default();
    }
    // `px`/`mx` are the *logical* inline axis in Tailwind
    // (`padding-inline`), not left/right. For a symmetric value the
    // rendering is identical either way, but keeping them logical matches
    // Tailwind's own output and composes correctly with `ps-*`/`pe-*`.
    if let Some(rest) = token.strip_prefix("px-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return vec![StyleProperty::PaddingInlineStart(v), StyleProperty::PaddingInlineEnd(v)];
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
        if let Some(v) = parse_margin_suffix(rest) {
            return vec![StyleProperty::MarginInlineStart(v.clone()), StyleProperty::MarginInlineEnd(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("my-") {
        if let Some(v) = parse_margin_suffix(rest) {
            return vec![StyleProperty::MarginTop(v.clone()), StyleProperty::MarginBottom(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("m-") {
        if let Some(v) = parse_margin_suffix(rest) {
            return vec![
                StyleProperty::MarginTop(v.clone()),
                StyleProperty::MarginRight(v.clone()),
                StyleProperty::MarginBottom(v.clone()),
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
    if let Some(props) = expand_dimension_family(token) {
        return props;
    }
    if let Some(props) = expand_filter(token) {
        return props;
    }
    if let Some(prop) = parse_keyword_utility(token) {
        return vec![prop];
    }
    if let Some(props) = parse_screen_reader_only(token) {
        return props;
    }
    if let Some(prop) = keyword_pair_utility(token) {
        return vec![prop];
    }
    if let Some(prop) = keyword_utility(token) {
        return vec![prop];
    }
    if let Some(rest) = token.strip_prefix("line-clamp-") {
        if rest == "none" {
            return vec![StyleProperty::LineClamp(None)];
        }
        if let Ok(lines) = rest.parse::<u32>() {
            return vec![StyleProperty::LineClamp(Some(Clamp::Lines(lines)))];
        }
    }
    if let Some(prop) = parse_extended_value(token) {
        return vec![prop];
    }
    if let Some(props) = parse_logical_border(token) {
        return props;
    }
    if let Some(prop) = expand_scrollbar(token) {
        return vec![prop];
    }
    if let Some(prop) = expand_mask(token) {
        return vec![prop];
    }
    if let Some(props) = expand_mask_gradient(token) {
        return props;
    }
    if let Some(props) = expand_gradient(token) {
        return props;
    }
    if let Some(props) = expand_scroll(token) {
        return props;
    }
    if let Some(props) = expand_paint(token) {
        return props;
    }
    if let Some(props) = expand_outline(token) {
        return props;
    }
    if let Some(props) = expand_divide(token) {
        return props;
    }
    // Before `expand_inset`, which would otherwise read `inset-ring-2` as
    // an inset of "ring-2".
    if let Some(prop) = parse_ring(token) {
        return vec![prop];
    }
    if let Some(props) = expand_inset(token) {
        return props;
    }
    if let Some(props) = expand_border_radius(token) {
        return props;
    }
    if let Some(rest) = token.strip_prefix("size-") {
        if let Some(d) = parse_dimension_suffix(rest) {
            return vec![StyleProperty::Width(d.clone()), StyleProperty::Height(d)];
        }
    }
    // A `transition-*` utility sets the property list *and* Tailwind's
    // default timing function and duration -- an explicit `duration-*` or
    // `ease-*` written after it then overrides those under last-wins
    // flattening, which is how Tailwind's own custom-property indirection
    // behaves.
    if let Some(properties) = parse_transition_properties(token) {
        // ...except `transition-none`, which turns transitions off. Tailwind
        // emits the property alone there; a timing function and duration
        // would be inert but would still be two declarations it didn't write.
        if token == "transition-none" {
            return vec![StyleProperty::TransitionProperty(properties.to_string())];
        }
        return vec![
            StyleProperty::TransitionProperty(properties.to_string()),
            StyleProperty::TransitionTimingFunction(DEFAULT_TRANSITION_TIMING.to_string()),
            StyleProperty::TransitionDuration(DEFAULT_TRANSITION_DURATION_MS),
        ];
    }
    // Three declarations, which is why it can't go through the
    // one-property path.
    if token == "truncate" {
        return vec![
            StyleProperty::Overflow(Overflow::Hidden),
            StyleProperty::TextOverflow(TextOverflow::Ellipsis),
            StyleProperty::WhiteSpace(WhiteSpace::NoWrap),
        ];
    }
    match token {
        "border-solid" => return all_sides_border_style(BorderStyle::Solid),
        "border-dashed" => return all_sides_border_style(BorderStyle::Dashed),
        "border-dotted" => return all_sides_border_style(BorderStyle::Dotted),
        "border-double" => return all_sides_border_style(BorderStyle::Double),
        "border-hidden" => return all_sides_border_style(BorderStyle::Hidden),
        "border-none" => return all_sides_border_style(BorderStyle::None),
        _ => {}
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
        // keeps leading-6, Hozo takes text-xl's 28px.
        return vec![StyleProperty::FontSize(size), StyleProperty::LineHeight(LineHeight::Length(line_height))];
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

    // Bare `border` == 1px on every side.
    if rest.is_empty() {
        return Some(all_sides_border(Length::Px(1.0)));
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

    // The style is scoped to the same side as the width. Setting it on all
    // four would make the other sides fall back to `border-width: medium`
    // and render, turning `border-t` into a full box.
    Some(match side {
        Some("t") => vec![
            StyleProperty::BorderTopWidth(width),
            StyleProperty::BorderTopStyle(BorderStyle::Solid),
        ],
        Some("r") => vec![
            StyleProperty::BorderRightWidth(width),
            StyleProperty::BorderRightStyle(BorderStyle::Solid),
        ],
        Some("b") => vec![
            StyleProperty::BorderBottomWidth(width),
            StyleProperty::BorderBottomStyle(BorderStyle::Solid),
        ],
        Some("l") => vec![
            StyleProperty::BorderLeftWidth(width),
            StyleProperty::BorderLeftStyle(BorderStyle::Solid),
        ],
        _ => all_sides_border(width),
    })
}

fn all_sides_border(width: Length) -> Vec<StyleProperty> {
    vec![
        StyleProperty::BorderTopWidth(width),
        StyleProperty::BorderRightWidth(width),
        StyleProperty::BorderBottomWidth(width),
        StyleProperty::BorderLeftWidth(width),
        StyleProperty::BorderTopStyle(BorderStyle::Solid),
        StyleProperty::BorderRightStyle(BorderStyle::Solid),
        StyleProperty::BorderBottomStyle(BorderStyle::Solid),
        StyleProperty::BorderLeftStyle(BorderStyle::Solid),
    ]
}

fn all_sides_border_style(style: BorderStyle) -> Vec<StyleProperty> {
    vec![
        StyleProperty::BorderTopStyle(style),
        StyleProperty::BorderRightStyle(style),
        StyleProperty::BorderBottomStyle(style),
        StyleProperty::BorderLeftStyle(style),
    ]
}

/// Border widths are plain pixel counts, not multiples of the spacing
/// scale -- `border-2` is 2px, not 8px.
fn parse_border_width_px(suffix: &str) -> Option<Length> {
    suffix.parse::<f64>().ok().map(Length::Px)
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
                    StyleProperty::PaddingTop(Length::Spacing(6.0)),
                    StyleProperty::PaddingRight(Length::Spacing(6.0)),
                    StyleProperty::PaddingBottom(Length::Spacing(6.0)),
                    StyleProperty::PaddingLeft(Length::Spacing(6.0)),
                ]
            )
        );
        assert_eq!(
            expand_utility("px-4"),
            (
                Condition::Always,
                vec![
                    StyleProperty::PaddingInlineStart(Length::Spacing(4.0)),
                    StyleProperty::PaddingInlineEnd(Length::Spacing(4.0))
                ]
            )
        );
        assert_eq!(
            expand_utility("text-xl"),
            (
                Condition::Always,
                vec![StyleProperty::FontSize(Length::Px(20.0)), StyleProperty::LineHeight(LineHeight::Length(Length::Px(28.0)))]
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
                vec![StyleProperty::FontSize(Length::Px(20.0)), StyleProperty::LineHeight(LineHeight::Length(Length::Px(28.0)))]
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
            (Condition::Always, vec![StyleProperty::InsetTop(Dimension::Length(Length::Spacing(4.0)))])
        );
        assert_eq!(
            expand_utility("inset-0"),
            (
                Condition::Always,
                vec![
                    StyleProperty::InsetTop(Dimension::Length(Length::Spacing(0.0))),
                    StyleProperty::InsetRight(Dimension::Length(Length::Spacing(0.0))),
                    StyleProperty::InsetBottom(Dimension::Length(Length::Spacing(0.0))),
                    StyleProperty::InsetLeft(Dimension::Length(Length::Spacing(0.0))),
                ]
            )
        );
    }

    #[test]
    fn border_width_always_carries_a_style_so_it_actually_renders() {
        // CSS defaults border-style to none, so a width with no style
        // renders nothing -- Tailwind pairs them for the same reason.
        let (_, props) = expand_utility("border");
        assert!(props.contains(&StyleProperty::BorderTopStyle(BorderStyle::Solid)));
        assert!(props.contains(&StyleProperty::BorderTopWidth(Length::Px(1.0))));
        assert_eq!(props.len(), 8); // 4 widths + 4 styles

        let (_, props) = expand_utility("border-2");
        assert!(props.contains(&StyleProperty::BorderLeftWidth(Length::Px(2.0))));
    }

    #[test]
    fn parses_display_including_the_web_only_values() {
        assert_eq!(
            expand_utility("hidden"),
            (Condition::Always, vec![StyleProperty::Display(Display::None)])
        );
        // Accepted at parse time even though Native can't lower it -- the
        // Web backend can, and hozo_native raises a build error naming it.
        assert_eq!(
            expand_utility("grid"),
            (Condition::Always, vec![StyleProperty::Display(Display::Grid)])
        );
        assert!(!Display::Grid.is_supported_on_native());
        assert!(Display::InlineFlex.is_supported_on_native());
        assert!(Display::None.is_supported_on_native());
    }

    #[test]
    fn parses_the_remaining_tier_one_utilities() {
        assert_eq!(
            expand_utility("z-10"),
            (Condition::Always, vec![StyleProperty::ZIndex(Some(10))])
        );
        assert_eq!(
            expand_utility("min-w-0"),
            (Condition::Always, vec![StyleProperty::MinWidth(Dimension::Length(Length::Spacing(0.0)))])
        );
        // max-w-* uses Tailwind's named container scale, not the spacing one.
        assert_eq!(
            expand_utility("max-w-md"),
            (Condition::Always, vec![StyleProperty::MaxWidth(Dimension::Length(Length::Px(448.0)))])
        );
        assert_eq!(
            expand_utility("self-center"),
            (Condition::Always, vec![StyleProperty::AlignSelf(AlignSelf::Center)])
        );
        assert_eq!(
            expand_utility("content-center"),
            (Condition::Always, vec![StyleProperty::AlignContent(Justify::Center)])
        );
        assert_eq!(
            expand_utility("uppercase"),
            (Condition::Always, vec![StyleProperty::TextTransform(TextTransform::Uppercase)])
        );
    }

    #[test]
    fn margins_accept_auto_where_padding_does_not() {
        assert_eq!(
            expand_utility("mx-auto"),
            (
                Condition::Always,
                vec![
                    StyleProperty::MarginInlineStart(Dimension::Auto),
                    StyleProperty::MarginInlineEnd(Dimension::Auto)
                ]
            )
        );
        assert_eq!(
            expand_utility("mt-auto"),
            (Condition::Always, vec![StyleProperty::MarginTop(Dimension::Auto)])
        );
        // Padding has no `auto` in CSS, so this stays unrecognized rather
        // than being invented.
        assert_eq!(expand_utility("pt-auto"), (Condition::Always, vec![]));
    }

    #[test]
    fn viewport_sizes_parse_as_a_viewport_dimension() {
        // Kept as a viewport dimension rather than resolved here, because
        // neither backend wants a number: Web writes `100vh` and lets the
        // browser resolve it, and Native reads the window at render time
        // (`hozo_native::viewport_object`). A pixel value baked in at
        // compile time would be wrong the moment the device rotated.
        let (_, props) = expand_utility("h-screen");
        assert_eq!(props, vec![StyleProperty::Height(Dimension::ViewportHeight(100.0))]);
        assert!(props[0].unsupported_on_native().is_none());
    }

    #[test]
    fn parses_effects_and_transforms() {
        assert_eq!(
            expand_utility("blur-sm"),
            (
                Condition::Always,
                vec![StyleProperty::Filter(FilterFunction::Blur, "blur(8px)".to_string())]
            )
        );
        assert_eq!(
            expand_utility("shadow-lg").1,
            vec![StyleProperty::BoxShadow(
                "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)".to_string()
            )]
        );
        assert_eq!(
            expand_utility("rotate-45"),
            (Condition::Always, vec![StyleProperty::Rotate(Angle::Deg(45.0))])
        );
        // Tailwind writes a percentage and the IR keeps it as authored.
        // Bare `scale-*` sets all three axes, so `scale-95 scale-x-50` can
        // resolve per-axis the way Tailwind does.
        assert_eq!(
            expand_utility("scale-95"),
            (
                Condition::Always,
                vec![
                    StyleProperty::ScaleX(Scale::Percent(95.0)),
                    StyleProperty::ScaleY(Scale::Percent(95.0)),
                    StyleProperty::ScaleZ(Scale::Percent(95.0)),
                ]
            )
        );
        assert_eq!(
            expand_utility("translate-x-2"),
            (Condition::Always, vec![StyleProperty::TranslateX(Dimension::Length(Length::Spacing(2.0)))])
        );
    }

    #[test]
    fn a_colour_slot_that_holds_a_keyword_does_not_go_through_the_palette() {
        // `accent-color: auto` and `fill: none` sit where a colour goes and
        // are not colours. Routing them through the token path produced
        // `var(--hozo-color-auto)` -- a plausible-looking wrong answer
        // rather than an unresolved one, which is the failure the palette
        // fallback exists to avoid in the first place.
        assert_eq!(
            expand_utility("accent-auto").1,
            vec![StyleProperty::AccentColor(Color::Keyword("auto"))]
        );
        assert_eq!(expand_utility("fill-none").1, vec![StyleProperty::Fill(Color::Keyword("none"))]);
    }

    #[test]
    fn sr_only_expands_into_the_properties_it_sets() {
        // Expanded rather than held as a blob so a `w-4` beside it still
        // overrides the width: `dedupe_key` can only do that if each
        // declaration arrives as its own property.
        let (_, props) = expand_utility("sr-only");
        assert!(props.contains(&StyleProperty::Position(Position::Absolute)));
        assert!(props.contains(&StyleProperty::Width(Dimension::Length(Length::Px(1.0)))));
        assert!(props.contains(&StyleProperty::Overflow(Overflow::Hidden)));
        assert!(props.contains(&StyleProperty::Keyword("clip-path", "inset(50%)")));
        // A clipped one-pixel box, not `display: none` -- those remove the
        // element from the accessibility tree, which is the opposite of
        // what this utility is for.
        assert!(!props.iter().any(|p| matches!(p, StyleProperty::Display(_))));
        // Widths only: a zero width needs no style to render nothing, and
        // Tailwind writes none.
        assert!(props.contains(&StyleProperty::BorderTopWidth(Length::Px(0.0))));
        assert!(!props.iter().any(|p| matches!(p, StyleProperty::BorderTopStyle(_))));

        let (_, props) = expand_utility("not-sr-only");
        assert!(props.contains(&StyleProperty::Position(Position::Static)));
        assert!(props.contains(&StyleProperty::Width(Dimension::Auto)));
    }

    #[test]
    fn line_clamp_is_one_property_because_react_native_has_one_prop() {
        assert_eq!(expand_utility("line-clamp-3").1, vec![StyleProperty::LineClamp(Some(Clamp::Lines(3)))]);
        assert_eq!(expand_utility("line-clamp-none").1, vec![StyleProperty::LineClamp(None)]);
    }

    #[test]
    fn position_static_is_not_refused() {
        // React Native has `static` and means what CSS means by it. It was
        // refused as part of the fixed/sticky group until the audit
        // objected.
        assert!(StyleProperty::Position(Position::Static).unsupported_on_native().is_none());
        assert!(StyleProperty::Position(Position::Css("sticky")).unsupported_on_native().is_some());
    }

    #[test]
    fn extra_values_use_the_real_variant_when_one_exists() {
        // The refusal audit caught this: routing every new alignment value
        // through `Css` refused `items-baseline` and `items-stretch` on
        // Native, where React Native's alignItems has both. A `Css`
        // fallthrough is for values the platform genuinely lacks, not a
        // shortcut past the variants that already exist.
        assert_eq!(
            expand_utility("items-baseline").1,
            vec![StyleProperty::AlignItems(Align::Baseline)]
        );
        assert_eq!(
            expand_utility("self-stretch").1,
            vec![StyleProperty::AlignSelf(AlignSelf::Stretch)]
        );
        assert!(StyleProperty::AlignItems(Align::Baseline).unsupported_on_native().is_none());

        // `safe` is an overflow-alignment prefix, and the flexbox spelling
        // of the edge comes with it: `safe flex-end`, not `safe end`.
        assert_eq!(
            expand_utility("justify-end-safe").1,
            vec![StyleProperty::JustifyContent(Justify::Css("safe flex-end"))]
        );

        // align-content and justify-content share `Justify`, so which
        // values React Native accepts is decided per property: it has
        // stretch on alignContent and on neither for justifyContent.
        assert!(StyleProperty::AlignContent(Justify::Stretch)
            .unsupported_on_native()
            .is_none());
        assert!(StyleProperty::JustifyContent(Justify::Stretch)
            .unsupported_on_native()
            .is_some());
    }

    #[test]
    fn underline_offset_is_plain_pixels_not_the_spacing_scale() {
        // `underline-offset-4` is 4px where `p-4` is 16px: an underline
        // offset is a typographic distance, not a layout step, so Tailwind
        // uses the bare number. Running it through the spacing scale
        // multiplied everything by four, which the oracle caught.
        assert_eq!(
            expand_utility("underline-offset-4").1,
            vec![StyleProperty::TextUnderlineOffset(Dimension::Length(Length::Px(4.0)))]
        );
        assert_eq!(
            expand_utility("-underline-offset-2").1,
            vec![StyleProperty::TextUnderlineOffset(Dimension::Length(Length::Px(-2.0)))]
        );
        assert_eq!(
            expand_utility("underline-offset-auto").1,
            vec![StyleProperty::TextUnderlineOffset(Dimension::Auto)]
        );
    }

    #[test]
    fn the_keyword_table_carries_tailwinds_own_values() {
        // Spot checks on details a table written from memory would smooth
        // over. Each of these came out of Tailwind's compiled output.
        for (token, property, value) in [
            // "safe" is a prefix on the alignment, not a separate keyword.
            ("place-content-center-safe", "place-content", "safe center"),
            // A percentage, not the keyword `left`.
            ("origin-left", "transform-origin", "0"),
            // Two values, in the order CSS wants them.
            ("bg-top-right", "background-position", "right top"),
            ("touch-pan-x", "touch-action", "pan-x"),
            ("scheme-only-dark", "color-scheme", "only dark"),
        ] {
            assert_eq!(
                expand_utility(token).1,
                vec![StyleProperty::Keyword(property, value)],
                "{token}"
            );
        }
    }

    #[test]
    fn no_keyword_property_is_also_written_by_a_variant() {
        // The other half of the `Keyword` invariant, and the half the
        // expansion test below cannot see: a token can expand to the
        // `Keyword` it claims and *still* collide, if some other utility
        // reaches the same CSS property through a variant. That is what
        // `z-auto` did -- it expanded to `Keyword("z-index", "auto")`
        // exactly as the table said, while `z-10` produced a `ZIndex`, so
        // the two stopped overriding each other.
        //
        // Checked by reading the Web emitter, because it is the only
        // exhaustive list of which CSS property each variant writes and
        // Rust can't enumerate enum variants. Coarse -- a property named in
        // a comment there would trip this -- but it fails loudly and in the
        // safe direction.
        let emitter = include_str!("../../hozo_web/src/css.rs");
        for (token, property, _) in KEYWORD_UTILITIES {
            // The quoted name anywhere, not `("name",`: a wide match arm
            // puts the property on its own line, so anchoring on the paren
            // silently missed every multi-line one -- which is most of the
            // interesting ones (`align-items`, `position`, `text-align`).
            assert!(
                !emitter.contains(&format!("\"{property}\"")),
                "`{token}` sets `{property}`, which hozo_web also writes from a variant: \
                 give it a property of its own instead of a Keyword"
            );
        }
    }

    #[test]
    fn keyword_table_avoids_the_modelled_properties() {
        // The invariant `StyleProperty::Keyword` documents: no property may
        // be reachable both as a `Keyword` and as a variant of its own,
        // because `dedupe_key` tells two `Keyword`s apart by property name
        // and cannot see that a `Keyword("scroll-behavior", ..)` and a
        // `ScrollBehavior(..)` are the same declaration. Writing both would
        // then emit both.
        //
        // Checked against the *Web emitter's* own answer rather than a
        // hand-kept list: `property_and_value` is what decides a variant's
        // CSS property, so asking it is the only way to be sure. This is
        // how `scroll-auto` was caught -- it slipped into the table because
        // `scroll-behavior` wasn't on the list I wrote by hand.
        for (token, property, _) in KEYWORD_UTILITIES {
            let (_, props) = expand_utility(token);
            assert_eq!(props.len(), 1, "{token} should be one declaration: {props:?}");
            assert!(
                matches!(props[0], StyleProperty::Keyword(p, _) if p == *property),
                "{token} is shadowed by a modelled utility: {:?}",
                props[0]
            );
        }
    }

    #[test]
    fn the_sizes_css_can_state_and_hozo_cannot_compute_are_kept_as_text() {
        // Every one of these is resolved by the browser against state the
        // compiler doesn't have -- an intrinsic content size, or a viewport
        // that tracks browser chrome. There is nothing to compute, so the
        // CSS text is carried through and React Native refuses it.
        assert_eq!(
            expand_utility("h-fit").1,
            vec![StyleProperty::Height(Dimension::Css("fit-content".to_string()))]
        );
        assert_eq!(
            expand_utility("max-w-dvh").1,
            vec![StyleProperty::MaxWidth(Dimension::Css("100dvh".to_string()))]
        );
        assert!(!Dimension::Css("fit-content".to_string()).is_supported_on_native());
        // `h-screen` is the one viewport size that *is* answerable on
        // Native, from `Dimensions` -- so it stays a viewport dimension
        // rather than joining these.
        assert_eq!(
            expand_utility("h-screen").1,
            vec![StyleProperty::Height(Dimension::ViewportHeight(100.0))]
        );
    }

    #[test]
    fn the_keyword_families_are_flat_tables() {
        // `flex-<n>` is `n 1 0%` -- grow by n, start from nothing -- which
        // is the same shape `flex-1` always had.
        assert_eq!(
            expand_utility("flex-7").1,
            vec![StyleProperty::Flex(FlexShorthand::Grow(7.0))]
        );
        assert_eq!(
            expand_utility("mix-blend-multiply").1,
            vec![StyleProperty::MixBlendMode("multiply")]
        );
        assert_eq!(
            expand_utility("bg-blend-screen").1,
            vec![StyleProperty::BackgroundBlendMode("screen")]
        );
        assert_eq!(expand_utility("mix-blend-nonsense").1, vec![]);

        assert_eq!(
            expand_utility("inline-block").1,
            vec![StyleProperty::Display(Display::Css("inline-block"))]
        );
        // `hidden` is Tailwind's name for `display: none`, which Yoga does
        // have -- so it must not land in the grouped keywords that get
        // refused on Native.
        assert_eq!(expand_utility("hidden").1, vec![StyleProperty::Display(Display::None)]);
        assert!(Display::None.is_supported_on_native());
        assert!(!Display::Css("inline-block").is_supported_on_native());
    }

    #[test]
    fn filter_functions_are_held_per_slot_so_they_compose() {
        // One property per function, not one string for the chain: holding
        // the whole chain would make `blur-md grayscale` last-wins instead
        // of composing, which is the same reason the ring and mask slots
        // are separate.
        assert_eq!(
            expand_utility("grayscale").1,
            vec![StyleProperty::Filter(FilterFunction::Grayscale, "grayscale(100%)".to_string())]
        );
        assert_eq!(
            expand_utility("hue-rotate-15").1,
            vec![StyleProperty::Filter(FilterFunction::HueRotate, "hue-rotate(15deg)".to_string())]
        );
        // The `-none` form clears one slot; Tailwind writes an empty
        // register for it, so it contributes nothing to the chain.
        assert_eq!(
            expand_utility("blur-none").1,
            vec![StyleProperty::Filter(FilterFunction::Blur, String::new())]
        );
        // `filter-none` is the whole chain off, which is a different thing.
        assert_eq!(
            expand_utility("filter-none").1,
            vec![StyleProperty::Filter(FilterFunction::None, String::new())]
        );

        // The backdrop forms are the same chain aimed at what's behind the
        // element, and an element can carry both.
        assert_eq!(
            expand_utility("backdrop-blur-md").1,
            vec![StyleProperty::BackdropFilter(FilterFunction::Blur, "blur(12px)".to_string())]
        );
        // `opacity()` is a filter function only in its backdrop form --
        // bare `opacity-50` is the CSS property.
        assert_eq!(
            expand_utility("backdrop-opacity-50").1,
            vec![StyleProperty::BackdropFilter(FilterFunction::Opacity, "opacity(50%)".to_string())]
        );
        assert_eq!(expand_utility("opacity-50").1, vec![StyleProperty::Opacity(0.5)]);
    }

    #[test]
    fn the_axis_transforms_are_separate_properties_so_they_can_override() {
        // The point of holding each axis separately: `dedupe_last_wins`
        // keys on the property, so a bare `scale-*` followed by an axis
        // has to leave two distinguishable properties or the axis would
        // replace the whole thing.
        assert_eq!(
            expand_utility("scale-x-50").1,
            vec![StyleProperty::ScaleX(Scale::Percent(50.0))]
        );
        // Writing the z axis also switches the declaration to its
        // three-value form, which is a separate fact from the axis having
        // a value.
        assert_eq!(
            expand_utility("scale-z-50").1,
            vec![StyleProperty::ScaleZ(Scale::Percent(50.0)), StyleProperty::Scale3d]
        );
        assert_eq!(expand_utility("scale-3d").1, vec![StyleProperty::Scale3d]);

        assert_eq!(
            expand_utility("rotate-x-45").1,
            vec![StyleProperty::RotateX(Angle::Deg(45.0))]
        );
        // Bare `skew-*` is both axes, the same way bare `scale-*` is all
        // three -- and must not be swallowed by the `skew-x-` branch.
        assert_eq!(
            expand_utility("skew-6").1,
            vec![
                StyleProperty::SkewX(Angle::Deg(6.0)),
                StyleProperty::SkewY(Angle::Deg(6.0))
            ]
        );
        assert_eq!(
            expand_utility("-rotate-y-12").1,
            vec![StyleProperty::RotateY(Angle::Deg(-12.0))]
        );
    }

    #[test]
    fn negated_transforms_are_recognized() {
        assert_eq!(
            expand_utility("-rotate-45"),
            (Condition::Always, vec![StyleProperty::Rotate(Angle::Deg(-45.0))])
        );
        assert_eq!(
            expand_utility("-translate-y-2"),
            (Condition::Always, vec![StyleProperty::TranslateY(Dimension::Length(Length::Spacing(-2.0)))])
        );
        assert_eq!(
            expand_utility("-translate-z-2"),
            (Condition::Always, vec![StyleProperty::TranslateZ(Dimension::Length(Length::Spacing(-2.0)))])
        );
        assert_eq!(expand_utility("-z-10"), (Condition::Always, vec![StyleProperty::ZIndex(Some(-10))]));
        assert_eq!(
            expand_utility("-outline-offset-2"),
            (Condition::Always, vec![StyleProperty::OutlineOffset(Length::Px(-2.0))])
        );
    }

    #[test]
    fn the_container_scale_is_inline_axis_only() {
        // Tailwind has no `max-h-md` or `h-2xl` -- a container is a measure
        // of line length, so a named one on the block axis means nothing.
        // Hozo accepted them until 2026-08-16, which the conformance report
        // structurally could not catch: a candidate Tailwind emits no rule
        // for isn't in the catalogue to compare against.
        let scale = Dimension::Length(Length::Px(448.0)); // --container-md
        for token in ["w-md", "min-w-md", "max-w-md", "basis-md", "inline-md", "max-inline-md"] {
            let (_, props) = expand_utility(token);
            assert_eq!(props.len(), 1, "{token}: {props:?}");
            assert!(
                matches!(
                    &props[0],
                    StyleProperty::Width(d)
                        | StyleProperty::MinWidth(d)
                        | StyleProperty::MaxWidth(d)
                        | StyleProperty::FlexBasis(d)
                        | StyleProperty::InlineSize(d)
                        | StyleProperty::MaxInlineSize(d) if *d == scale
                ),
                "{token}: {props:?}"
            );
        }
        for token in ["h-md", "max-h-md", "min-h-2xl", "block-md", "max-block-md"] {
            assert_eq!(expand_utility(token), (Condition::Always, vec![]), "{token}");
        }
    }

    #[test]
    fn order_cursor_and_columns_are_plain_tables() {
        assert_eq!(expand_utility("order-3"), (Condition::Always, vec![StyleProperty::Order(3)]));
        // Tailwind's own sentinels, not CSS keywords: `order` has none, so
        // "first" is a number far enough out that nothing outranks it.
        assert_eq!(
            expand_utility("order-first"),
            (Condition::Always, vec![StyleProperty::Order(-9999)])
        );
        assert_eq!(expand_utility("-order-3"), (Condition::Always, vec![StyleProperty::Order(-3)]));

        assert_eq!(
            expand_utility("cursor-pointer"),
            (Condition::Always, vec![StyleProperty::Cursor("pointer".to_string())])
        );
        // The keyword list decides which names are utilities at all --
        // passing anything through would compile to CSS the browser drops.
        assert_eq!(expand_utility("cursor-nonsense"), (Condition::Always, vec![]));

        // A count and a width mean opposite things, so the two forms stay
        // distinguishable in the IR.
        assert_eq!(
            expand_utility("columns-3"),
            (Condition::Always, vec![StyleProperty::Columns(ColumnCount::Count(3))])
        );
        assert_eq!(
            expand_utility("columns-md"),
            (
                Condition::Always,
                vec![StyleProperty::Columns(ColumnCount::Width(Dimension::Length(Length::Px(
                    448.0
                ))))]
            )
        );
    }

    #[test]
    fn direction_relative_utilities_stay_logical() {
        // These are the ones that actually flip between LTR and RTL, so
        // they must not be resolved to a physical side at compile time --
        // which side "start" is isn't known until runtime.
        assert_eq!(
            expand_utility("ps-4"),
            (Condition::Always, vec![StyleProperty::PaddingInlineStart(Length::Spacing(4.0))])
        );
        assert_eq!(
            expand_utility("me-2"),
            (Condition::Always, vec![StyleProperty::MarginInlineEnd(Dimension::Length(Length::Spacing(2.0)))])
        );
        assert_eq!(
            expand_utility("start-2"),
            (Condition::Always, vec![StyleProperty::InsetInlineStart(Dimension::Length(Length::Spacing(2.0)))])
        );
        // The physical ones stay physical -- Tailwind has both families.
        assert_eq!(
            expand_utility("pl-4"),
            (Condition::Always, vec![StyleProperty::PaddingLeft(Length::Spacing(4.0))])
        );
        assert_eq!(
            expand_utility("left-2"),
            (Condition::Always, vec![StyleProperty::InsetLeft(Dimension::Length(Length::Spacing(2.0)))])
        );
    }

    #[test]
    fn mask_keyword_utilities_map_to_their_own_property() {
        // The family gives no hint from the name which property a keyword
        // belongs to, which is why the parser is a flat table.
        assert_eq!(expand_utility("mask-center").1, vec![StyleProperty::MaskPosition("center")]);
        assert_eq!(expand_utility("mask-cover").1, vec![StyleProperty::MaskSize("cover")]);
        assert_eq!(expand_utility("mask-alpha").1, vec![StyleProperty::MaskMode("alpha")]);
        assert_eq!(
            expand_utility("mask-clip-content").1,
            vec![StyleProperty::MaskClip("content-box")]
        );
        assert_eq!(expand_utility("mask-none").1, vec![StyleProperty::MaskImageNone]);
    }

    #[test]
    fn stacked_variants_become_one_condition_each() {
        // These compiled to nothing at all before: the first prefix was
        // stripped and `hover:bg-blue-500` was handed to the utility
        // parser, which has never heard of it.
        assert_eq!(
            expand_utility("md:hover:flex").0,
            Condition::All(vec![Condition::Responsive(Breakpoint::Md), Condition::Hover])
        );
        // Written order is kept, because it is the order the at-rules nest
        // in and Tailwind nests them the same way.
        assert_eq!(
            expand_utility("hover:md:flex").0,
            Condition::All(vec![Condition::Hover, Condition::Responsive(Breakpoint::Md)])
        );
        assert_eq!(
            expand_utility("md:first:last:flex").0,
            Condition::All(vec![
                Condition::Responsive(Breakpoint::Md),
                Condition::FirstChild,
                Condition::LastChild,
            ])
        );
        // One variant stays what it always was, so everything matching on
        // `Condition::Hover` keeps working.
        assert_eq!(expand_utility("hover:flex").0, Condition::Hover);
    }

    #[test]
    fn an_unrecognised_variant_stops_the_scan() {
        // Rather than being skipped: skipping it would apply the style
        // unconditionally, which is a wrong answer where nothing is an
        // honest gap. The colon may also belong to the value.
        assert_eq!(expand_utility("supports-grid:flex").1, Vec::<StyleProperty>::new());
    }

    #[test]
    fn container_is_six_utilities_wearing_one_name() {
        let groups = expand_class("container");
        assert_eq!(groups.len(), 6);
        assert_eq!(groups[0].0, Condition::Always);
        assert_eq!(groups[0].1, vec![StyleProperty::Width(Dimension::Percent(100.0))]);
        assert_eq!(groups[1].0, Condition::Responsive(Breakpoint::Sm));
        assert_eq!(
            groups[1].1,
            vec![StyleProperty::MaxWidth(Dimension::Length(Length::Unit(
                40.0,
                hozo_ir::LengthUnit::Rem
            )))]
        );
        // The variant rides onto every piece, so the two width queries
        // nest the way Tailwind's do.
        assert_eq!(
            expand_class("md:container")[1].0,
            Condition::All(vec![
                Condition::Responsive(Breakpoint::Md),
                Condition::Responsive(Breakpoint::Sm),
            ])
        );
    }

    #[test]
    fn the_variants_tailwind_spells_differently() {
        assert_eq!(expand_utility("last:flex").0, Condition::LastChild);
        assert_eq!(expand_utility("focus-visible:flex").0, Condition::FocusVisible);
        // Hozo's own name and Tailwind's for the same state.
        assert_eq!(expand_utility("active:flex").0, Condition::Pressed);
        assert_eq!(expand_utility("pressed:flex").0, Condition::Pressed);
    }

    #[test]
    fn a_gradient_constructor_carries_its_whole_prelude() {
        // The interpolation space is part of it, and Tailwind's default is
        // Oklab rather than sRGB -- which is why its red-to-blue ramps
        // don't go through grey.
        assert_eq!(
            expand_utility("bg-linear-to-r").1,
            vec![StyleProperty::Gradient(GradientKind::Linear, "to right in oklab".to_string())]
        );
        assert_eq!(
            expand_utility("bg-linear-45").1,
            vec![StyleProperty::Gradient(GradientKind::Linear, "45deg in oklab".to_string())]
        );
        assert_eq!(
            expand_utility("bg-conic-90").1,
            vec![StyleProperty::Gradient(GradientKind::Conic, "from 90deg in oklab".to_string())]
        );
        // No direction at all, so the prelude is only the interpolation.
        assert_eq!(
            expand_utility("bg-radial").1,
            vec![StyleProperty::Gradient(GradientKind::Radial, "in oklab".to_string())]
        );
        // The modifier replaces the space; a hue method brings one with it.
        assert_eq!(
            expand_utility("bg-linear-to-r/srgb").1,
            vec![StyleProperty::Gradient(GradientKind::Linear, "to right in srgb".to_string())]
        );
        assert_eq!(
            expand_utility("bg-linear-to-r/longer").1,
            vec![StyleProperty::Gradient(
                GradientKind::Linear,
                "to right in oklch longer hue".to_string()
            )]
        );
    }

    #[test]
    fn a_gradient_stop_is_a_colour_or_a_position() {
        assert_eq!(
            expand_utility("from-red-500").1,
            vec![StyleProperty::GradientStopColor(
                GradientStop::From,
                Color::Token("red-500".to_string())
            )]
        );
        assert_eq!(
            expand_utility("via-30%").1,
            vec![StyleProperty::GradientStopPosition(
                GradientStop::Via,
                Dimension::Percent(30.0)
            )]
        );
    }

    #[test]
    fn ring_offset_is_its_own_layer() {
        // Not a ring colour called `offset-white`, which is what reading it
        // as part of the ring would give.
        assert_eq!(
            expand_utility("ring-offset-2").1,
            vec![StyleProperty::RingOffsetWidth(Length::Px(2.0))]
        );
        assert_eq!(
            expand_utility("ring-offset-white").1,
            vec![StyleProperty::RingOffsetColor(Color::Token("white".to_string()))]
        );
    }

    #[test]
    fn a_shadow_suffix_the_size_table_declines_is_a_colour() {
        assert_eq!(
            expand_utility("shadow-blue-500").1,
            vec![StyleProperty::ShadowColor(Color::Token("blue-500".to_string()))]
        );
        // And the sizes still win, since they are tried first.
        assert_eq!(
            expand_utility("shadow-lg").1,
            vec![StyleProperty::BoxShadow(
                "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)".to_string()
            )]
        );
    }

    #[test]
    fn mask_gradients_name_their_slot_and_stop() {
        assert_eq!(
            expand_utility("mask-t-from-4").1,
            vec![StyleProperty::MaskStopPosition(
                MaskSlot::Top,
                MaskStop::From,
                Dimension::Length(Length::Spacing(4.0))
            )]
        );
        assert_eq!(
            expand_utility("mask-radial-to-red-500").1,
            vec![StyleProperty::MaskStopColor(
                MaskSlot::Radial,
                MaskStop::To,
                Color::Token("red-500".to_string())
            )]
        );
        // An axis names two slots, exactly as Tailwind does.
        assert_eq!(
            expand_utility("mask-x-from-4").1,
            vec![
                StyleProperty::MaskStopPosition(
                    MaskSlot::Left,
                    MaskStop::From,
                    Dimension::Length(Length::Spacing(4.0))
                ),
                StyleProperty::MaskStopPosition(
                    MaskSlot::Right,
                    MaskStop::From,
                    Dimension::Length(Length::Spacing(4.0))
                ),
            ]
        );
        assert_eq!(
            expand_utility("-mask-linear-45").1,
            vec![StyleProperty::MaskSlotArgument(MaskSlot::Linear, Angle::Deg(-45.0))]
        );
        assert_eq!(
            expand_utility("mask-subtract").1,
            vec![StyleProperty::MaskComposite("subtract")]
        );
    }

    #[test]
    fn a_leading_minus_is_handled_once_for_every_family() {
        // `expand_negatable` flips the expanded result rather than each
        // parser growing its own sign handling.
        assert_eq!(
            expand_utility("-mt-4").1,
            vec![StyleProperty::MarginTop(Dimension::Length(Length::Spacing(-4.0)))]
        );
        assert_eq!(
            expand_utility("-top-4").1,
            vec![StyleProperty::InsetTop(Dimension::Length(Length::Spacing(-4.0)))]
        );
        assert_eq!(expand_utility("-rotate-45").1, vec![StyleProperty::Rotate(Angle::Deg(-45.0))]);
        // ...and refuses where CSS has no negative, rather than inventing
        // a negative padding.
        assert!(expand_utility("-p-4").1.is_empty());
        assert!(expand_utility("-scroll-p-4").1.is_empty());
    }

    #[test]
    fn dimension_families_accept_fractions_and_keywords() {
        assert_eq!(
            expand_utility("top-1/2").1,
            vec![StyleProperty::InsetTop(Dimension::Percent(50.0))]
        );
        assert_eq!(
            expand_utility("basis-1/3").1,
            vec![StyleProperty::FlexBasis(Dimension::Percent(1.0 / 3.0 * 100.0))]
        );
        assert_eq!(expand_utility("basis-auto").1, vec![StyleProperty::FlexBasis(Dimension::Auto)]);
        // Longest prefix wins: `max-block-` must not be read as `block-`.
        assert_eq!(
            expand_utility("max-block-4").1,
            vec![StyleProperty::MaxBlockSize(Dimension::Length(Length::Spacing(4.0)))]
        );
        // Bare `translate-*` sets both axes.
        assert_eq!(
            expand_utility("translate-1/2").1,
            vec![
                StyleProperty::TranslateX(Dimension::Percent(50.0)),
                StyleProperty::TranslateY(Dimension::Percent(50.0)),
            ]
        );
    }

    #[test]
    fn scroll_margin_and_padding_cover_every_edge() {
        assert_eq!(
            expand_utility("scroll-mt-4").1,
            vec![StyleProperty::ScrollMargin(Edge::Top, Length::Spacing(4.0))]
        );
        assert_eq!(
            expand_utility("scroll-pe-2").1,
            vec![StyleProperty::ScrollPadding(Edge::InlineEnd, Length::Spacing(2.0))]
        );
        assert_eq!(
            expand_utility("-scroll-mx-4").1,
            vec![StyleProperty::ScrollMargin(Edge::Inline, Length::Spacing(-4.0))]
        );
        // Padding takes no negative value, in CSS or in Tailwind.
        assert!(expand_utility("-scroll-p-4").1.is_empty());
        // Both values live in the same variant. `scroll-auto` used to be
        // left unsupported -- a coverage gap justified as a choice, since
        // it *is* the initial value -- but Tailwind emits a declaration for
        // it, and once it lowered it had to lower here rather than as a
        // `Keyword`, or `scroll-smooth scroll-auto` would emit both.
        assert_eq!(
            expand_utility("scroll-smooth").1,
            vec![StyleProperty::ScrollBehavior("smooth")]
        );
        assert_eq!(expand_utility("scroll-auto").1, vec![StyleProperty::ScrollBehavior("auto")]);
    }

    #[test]
    fn per_side_border_colors_reach_the_right_side() {
        // Until 2026-08-15 every one of these compiled to `border-color`
        // -- all four sides -- from a token that isn't a colour name
        // (`var(--hozo-color-b-red-500)`). Wrong property, wrong value.
        assert_eq!(
            expand_utility("border-b-red-500").1,
            vec![StyleProperty::BorderBottomColor(Color::Token("red-500".to_string()))]
        );
        assert_eq!(
            expand_utility("border-s-red-500").1,
            vec![StyleProperty::BorderInlineStartColor(Color::Token("red-500".to_string()))]
        );
        // The axis forms stay shorthands, which is what Tailwind emits.
        assert_eq!(
            expand_utility("border-x-red-500").1,
            vec![StyleProperty::BorderInlineColor(Color::Token("red-500".to_string()))]
        );
        // A number on a side is still a width, not a colour called "4".
        assert_eq!(
            expand_utility("border-b-4").1,
            vec![
                StyleProperty::BorderBottomWidth(Length::Px(4.0)),
                StyleProperty::BorderBottomStyle(BorderStyle::Solid),
            ]
        );
    }

    #[test]
    fn inset_covers_its_axis_and_logical_forms_and_negatives() {
        assert_eq!(
            expand_utility("inset-x-4").1,
            vec![StyleProperty::InsetInline(Dimension::Length(Length::Spacing(4.0)))]
        );
        assert_eq!(
            expand_utility("inset-bs-2").1,
            vec![StyleProperty::InsetBlockStart(Dimension::Length(Length::Spacing(2.0)))]
        );
        assert_eq!(
            expand_utility("-inset-y-4").1,
            vec![StyleProperty::InsetBlock(Dimension::Length(Length::Spacing(-4.0)))]
        );
        // Bare `inset-*` is still all four physical sides.
        assert_eq!(expand_utility("inset-0").1.len(), 4);
    }

    #[test]
    fn rounded_corners_expand_to_the_longhands_tailwind_emits() {
        let lg = Radius::Length(Length::Px(8.0));
        assert_eq!(
            expand_utility("rounded-t-lg").1,
            vec![
                StyleProperty::BorderTopLeftRadius(lg),
                StyleProperty::BorderTopRightRadius(lg),
            ]
        );
        assert_eq!(
            expand_utility("rounded-tl-lg").1,
            vec![StyleProperty::BorderTopLeftRadius(lg)]
        );
        // Logical corners stay logical, so RTL keeps working.
        assert_eq!(
            expand_utility("rounded-s-lg").1,
            vec![
                StyleProperty::BorderStartStartRadius(lg),
                StyleProperty::BorderEndStartRadius(lg),
            ]
        );
        // The all-corners form is unaffected.
        assert_eq!(expand_utility("rounded-lg").1, vec![StyleProperty::BorderRadius(lg)]);
    }

    #[test]
    fn per_side_border_scopes_its_style_to_that_side() {
        // The important part: NOT an all-sides `border-style`. That would
        // leave the other three sides styled but width-less, so CSS's
        // `border-width: medium` initial value kicks in and draws them --
        // turning `border-t` into a full box.
        assert_eq!(
            expand_utility("border-t").1,
            vec![
                StyleProperty::BorderTopWidth(Length::Px(1.0)),
                StyleProperty::BorderTopStyle(BorderStyle::Solid)
            ]
        );
        assert_eq!(
            expand_utility("border-b-4").1,
            vec![
                StyleProperty::BorderBottomWidth(Length::Px(4.0)),
                StyleProperty::BorderBottomStyle(BorderStyle::Solid)
            ]
        );
    }

    #[test]
    fn standalone_border_style_utilities_cover_all_sides() {
        assert_eq!(
            expand_utility("border-dashed").1,
            vec![
                StyleProperty::BorderTopStyle(BorderStyle::Dashed),
                StyleProperty::BorderRightStyle(BorderStyle::Dashed),
                StyleProperty::BorderBottomStyle(BorderStyle::Dashed),
                StyleProperty::BorderLeftStyle(BorderStyle::Dashed),
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
            (Condition::Always, vec![StyleProperty::BorderRadius(Radius::Length(Length::Px(8.0)))])
        );
        assert_eq!(
            expand_utility("rounded"),
            (Condition::Always, vec![StyleProperty::BorderRadius(Radius::Length(Length::Px(4.0)))])
        );
    }

    #[test]
    fn rounded_full_stays_an_intent_not_a_number() {
        // Each backend needs a different answer -- CSS can say `infinity`,
        // RN can't -- so the choice can't be baked in at parse time.
        assert_eq!(
            expand_utility("rounded-full"),
            (Condition::Always, vec![StyleProperty::BorderRadius(Radius::Full)])
        );
    }

    #[test]
    fn parses_sizing_including_fractions_and_size_shorthand() {
        assert_eq!(
            expand_utility("w-4"),
            (Condition::Always, vec![StyleProperty::Width(Dimension::Length(Length::Spacing(4.0)))])
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
                    StyleProperty::Width(Dimension::Length(Length::Spacing(4.0))),
                    StyleProperty::Height(Dimension::Length(Length::Spacing(4.0))),
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
                        StyleProperty::LineHeight(LineHeight::Length(Length::Px(line_height))),
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
                StyleProperty::LineHeight(LineHeight::Length(Length::Px(48.0)))
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
        // Hozo resolves this by source order (last wins), so `leading-*`
        // must be written after `text-*` to take effect. Real Tailwind is
        // order-independent here (it routes leading through a --tw-leading
        // custom property) -- a known, documented divergence.
        let (_, text_props) = expand_utility("text-xl");
        let (_, leading_props) = expand_utility("leading-6");
        let combined: Vec<_> = text_props.into_iter().chain(leading_props).collect();
        let deduped = hozo_ir::dedupe_last_wins(combined);
        assert!(deduped.contains(&StyleProperty::LineHeight(LineHeight::Length(Length::Spacing(6.0)))));
        assert!(!deduped.contains(&StyleProperty::LineHeight(LineHeight::Length(Length::Px(28.0)))));
    }

    #[test]
    fn named_leading_stays_a_ratio_rather_than_being_faked_as_pixels() {
        // The named scale is a unitless multiple of the element's own font
        // size. It's kept as a ratio -- which CSS states directly -- rather
        // than converted here to a pixel value that would only be right for
        // one font size. The Native backend resolves it against a font size
        // on the same element (`hozo_native::fold_font_relative`), which is
        // information this function doesn't have.
        let (_, props) = expand_utility("leading-tight");
        assert_eq!(props, vec![StyleProperty::LineHeight(LineHeight::Ratio(1.25))]);

        // The numeric scale is spacing-based and resolves to a length on
        // both platforms.
        let (_, props) = expand_utility("leading-6");
        assert_eq!(props, vec![StyleProperty::LineHeight(LineHeight::Length(Length::Spacing(6.0)))]);
        assert!(props[0].unsupported_on_native().is_none());
    }

    #[test]
    fn truncate_expands_to_its_three_declarations() {
        assert_eq!(
            expand_utility("truncate").1,
            vec![
                StyleProperty::Overflow(Overflow::Hidden),
                StyleProperty::TextOverflow(TextOverflow::Ellipsis),
                StyleProperty::WhiteSpace(WhiteSpace::NoWrap),
            ]
        );
    }

    #[test]
    fn parses_dark_and_first_variants() {
        assert_eq!(
            expand_utility("dark:bg-black"),
            (Condition::Dark, vec![StyleProperty::BackgroundColor(Color::Token("black".to_string()))])
        );
        assert_eq!(
            expand_utility("first:mt-0"),
            (Condition::FirstChild, vec![StyleProperty::MarginTop(Dimension::Length(Length::Spacing(0.0)))])
        );
    }

    #[test]
    fn parses_transition_and_tracking() {
        assert_eq!(
            expand_utility("duration-200").1,
            vec![StyleProperty::TransitionDuration(200)]
        );
        assert_eq!(
            expand_utility("tracking-wide").1,
            vec![StyleProperty::LetterSpacing(LetterSpacing::Em(Em(0.025)))]
        );
        assert_eq!(
            expand_utility("grid-cols-3").1,
            vec![StyleProperty::GridTemplateColumns(GridTracks::Count(3))]
        );
    }

    #[test]
    fn grid_placement_distinguishes_a_line_from_a_span() {
        // `col-start-2` pins one edge to line 2; `col-span-2` says "two
        // tracks, wherever this lands". Same digit, different meaning, and
        // CSS spells them with different properties -- so collapsing them
        // would put an item in the wrong place rather than merely format it
        // differently.
        assert_eq!(
            expand_utility("col-start-2").1,
            vec![StyleProperty::GridColumnStart(GridLine::Line(2))]
        );
        assert_eq!(
            expand_utility("col-span-2").1,
            vec![StyleProperty::GridColumn(GridSpan::Span(2))]
        );
        // The shorthands must be matched before the bare-line form, or
        // `col-span-2` would parse as the `col-` line `span-2` and fail.
        assert_eq!(
            expand_utility("row-span-full").1,
            vec![StyleProperty::GridRow(GridSpan::Full)]
        );
        assert_eq!(expand_utility("col-auto").1, vec![StyleProperty::GridColumn(GridSpan::Auto)]);
        assert_eq!(
            expand_utility("col-end-auto").1,
            vec![StyleProperty::GridColumnEnd(GridLine::Auto)]
        );
        // A negative line counts back from the end of the explicit grid.
        assert_eq!(
            expand_utility("-col-end-1").1,
            vec![StyleProperty::GridColumnEnd(GridLine::Line(-1))]
        );
        assert_eq!(
            expand_utility("grid-rows-subgrid").1,
            vec![StyleProperty::GridTemplateRows(GridTracks::Subgrid)]
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
