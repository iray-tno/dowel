//! Maps a single Tailwind utility class token to a `dowel_ir::StyleProperty`.
//!
//! Phase 0 scope only (proposal §13): flex layout, spacing, color,
//! typography. Unrecognized tokens return `None` rather than erroring --
//! callers decide what to do with an unmapped utility (Phase 0: drop it).

use dowel_ir::{
    Align, AlignSelf, Angle, Animation, BorderStyle, Breakpoint, Color, Condition, Dimension,
    DecorationStyle, Display, Edge, Em,
    FlexDirection, FlexShorthand, FontWeight, Justify, Length, LineHeight, Overflow, Position,
    Radius, StyleProperty, TextAlign, TextOverflow, TextTransform, WhiteSpace,
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
        "transition-none" => "none",
        _ => return None,
    })
}

/// Tailwind's default spacing scale: `spacing(n) = n * 0.25rem`, and the
/// default root font size is 16px, so each spacing step is 4px.
fn spacing_to_px(n: f64) -> Length {
    Length::Px(n * 4.0)
}

fn parse_spacing_suffix(suffix: &str) -> Option<Length> {
    if suffix == "px" {
        return Some(Length::Px(1.0));
    }
    suffix.parse::<f64>().ok().map(spacing_to_px)
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
            return Some(StyleProperty::LetterSpacing(Em(em)));
        }
    }
    if let Some(rest) = token.strip_prefix("duration-") {
        if let Ok(ms) = rest.parse::<u32>() {
            return Some(StyleProperty::TransitionDuration(ms));
        }
    }
    if let Some(rest) = token.strip_prefix("grid-cols-") {
        if let Ok(n) = rest.parse::<u32>() {
            return Some(StyleProperty::GridTemplateColumns(n));
        }
    }
    if let Some(rest) = token.strip_prefix("space-x-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::SpaceX(v));
        }
    }
    if let Some(rest) = token.strip_prefix("space-y-") {
        if let Some(v) = parse_spacing_suffix(rest) {
            return Some(StyleProperty::SpaceY(v));
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
    if let Some(rest) = token.strip_prefix("min-w-") {
        if let Some(d) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::MinWidth(d));
        }
    }
    if let Some(rest) = token.strip_prefix("min-h-") {
        if let Some(d) = parse_dimension_suffix(rest) {
            return Some(StyleProperty::MinHeight(d));
        }
    }
    if let Some(rest) = token.strip_prefix("max-w-") {
        if let Some(d) = parse_max_size_suffix(rest) {
            return Some(StyleProperty::MaxWidth(d));
        }
    }
    if let Some(rest) = token.strip_prefix("max-h-") {
        if let Some(d) = parse_max_size_suffix(rest) {
            return Some(StyleProperty::MaxHeight(d));
        }
    }
    if let Some(rest) = token.strip_prefix("z-") {
        if let Ok(z) = rest.parse::<i32>() {
            return Some(StyleProperty::ZIndex(z));
        }
    }
    if let Some(shadow) = parse_shadow(token) {
        return Some(StyleProperty::BoxShadow(shadow.to_string()));
    }
    if let Some(blur) = parse_blur(token) {
        return Some(StyleProperty::Filter(format!("blur({blur}px)")));
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
    let (negative, token) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    let rest = token.strip_prefix("scroll-")?;

    // `scroll-auto` is the initial value, so it emits nothing meaningful on
    // its own and is left unsupported rather than approximated.
    if rest == "smooth" {
        return Some(vec![StyleProperty::ScrollBehaviorSmooth]);
    }

    let family = rest.chars().next()?;
    let (edge_part, value) = rest.get(1..)?.split_once('-')?;
    let edge = edge_keyword(edge_part)?;
    let Length::Px(px) = parse_spacing_suffix(value)?;
    let length = Length::Px(signed(px, negative));

    Some(match family {
        'm' => vec![StyleProperty::ScrollMargin(edge, length)],
        // Padding takes no negative value, in CSS or in Tailwind.
        'p' if !negative => vec![StyleProperty::ScrollPadding(edge, length)],
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
        // Thickness keywords Dowel doesn't lower. Declining leaves them
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
    if let Some(style) = border_style_keyword(suffix) {
        return Some(vec![StyleProperty::OutlineStyle(style)]);
    }
    if let Ok(px) = suffix.parse::<f64>() {
        return Some(vec![
            StyleProperty::OutlineStyle(BorderStyle::Solid),
            StyleProperty::OutlineWidth(Length::Px(px)),
        ]);
    }
    // `outline-hidden` is Tailwind's accessible "no visible outline, but
    // keep one for forced-colors mode"; it emits nothing standalone, so
    // it's left unsupported rather than approximated as `none`.
    if suffix == "hidden" {
        return None;
    }
    Some(vec![StyleProperty::OutlineColor(Color::Token(suffix.to_string()))])
}

/// `divide-*`: the border between an element's children. Shares the
/// child-scoped rule mechanism with `space-*`.
fn expand_divide(token: &str) -> Option<Vec<StyleProperty>> {
    let suffix = token.strip_prefix("divide-")?;

    if let Some(width) = suffix.strip_prefix("x") {
        if let Some(length) = divide_width(width) {
            return Some(vec![StyleProperty::DivideX(length)]);
        }
    }
    if let Some(width) = suffix.strip_prefix("y") {
        if let Some(length) = divide_width(width) {
            return Some(vec![StyleProperty::DivideY(length)]);
        }
    }
    if let Some(style) = border_style_keyword(suffix) {
        return Some(vec![StyleProperty::DivideStyle(style)]);
    }
    // `divide-x-reverse` only flips which edge the border sits on, through
    // a custom property Dowel doesn't emit; it paints nothing standalone.
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
    // `ring-offset-*` is a third layer with its own colour and width, not a
    // spelling of the ring itself. Declining leaves it unsupported by name
    // rather than mis-read as a ring colour called `offset-red-500`.
    if rest.starts_with("-offset") {
        return None;
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
    let (negative, token) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    let rest = token.strip_prefix("inset-")?;

    let (side, value) = match rest.split_once('-') {
        Some((side, value)) if matches!(side, "x" | "y" | "s" | "e" | "bs" | "be") => {
            (Some(side), value)
        }
        _ => (None, rest),
    };
    let Length::Px(px) = parse_spacing_suffix(value)?;
    let length = Length::Px(signed(px, negative));

    Some(match side {
        Some("x") => vec![StyleProperty::InsetInline(length)],
        Some("y") => vec![StyleProperty::InsetBlock(length)],
        Some("s") => vec![StyleProperty::InsetInlineStart(length)],
        Some("e") => vec![StyleProperty::InsetInlineEnd(length)],
        Some("bs") => vec![StyleProperty::InsetBlockStart(length)],
        Some("be") => vec![StyleProperty::InsetBlockEnd(length)],
        _ => vec![
            StyleProperty::InsetTop(length),
            StyleProperty::InsetRight(length),
            StyleProperty::InsetBottom(length),
            StyleProperty::InsetLeft(length),
        ],
    })
}

/// `border-<side>-<color>`, for every side Tailwind offers.
///
/// The width forms (`border-t-2`) matched an earlier arm, so a side here is
/// always followed by a colour token. Until 2026-08-15 this fell through to
/// the plain colour path and compiled `border-b-red-500` to
/// `border-color: var(--dowel-color-b-red-500)` -- the wrong property, on
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
/// `var(--dowel-color-<token>)` so a project's own theme colour still
/// reaches CSS. That is the right behaviour for `bg-brand-primary`, and
/// quietly wrong for everything else -- `bg-auto` is a background *size*,
/// and it was compiling to `background-color: var(--dowel-color-auto)`, a
/// custom property nothing defines. Inert output, and Dowel claiming the
/// utility while producing it.
///
/// So the catch-all is now guarded by the list below, derived from
/// Tailwind's own class list by asking which entries in each family it does
/// *not* give a colour property to (523 candidates did this). Matching one
/// means "not supported yet", not "not a colour utility Dowel will ever
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
            // `border-color: var(--dowel-color-b-red-500)`) -- plus the
            // logical/axis widths Dowel doesn't lower yet.
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
    // Kept as an intent rather than a number -- see `dowel_ir::Radius`.
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
/// no-op) unless a `ring-*` utility is present -- which Dowel doesn't
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

/// `rotate-<deg>`, `scale-<pct>`, `translate-x-<n>`, `translate-y-<n>`,
/// each optionally negated with a leading `-`.
fn parse_transform(token: &str) -> Option<StyleProperty> {
    let (negative, token) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    if let Some(rest) = token.strip_prefix("rotate-") {
        let degrees: f64 = rest.parse().ok()?;
        return Some(StyleProperty::Rotate(Angle { degrees: signed(degrees, negative) }));
    }
    if let Some(rest) = token.strip_prefix("scale-") {
        let pct: f64 = rest.parse().ok()?;
        // Kept as the authored percentage -- see `StyleProperty::Scale`.
        return Some(StyleProperty::Scale(signed(pct, negative)));
    }
    if let Some(rest) = token.strip_prefix("translate-x-") {
        let Length::Px(v) = parse_spacing_suffix(rest)?;
        return Some(StyleProperty::TranslateX(Length::Px(signed(v, negative))));
    }
    if let Some(rest) = token.strip_prefix("translate-y-") {
        let Length::Px(v) = parse_spacing_suffix(rest)?;
        return Some(StyleProperty::TranslateY(Length::Px(signed(v, negative))));
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

/// `max-w-*`/`max-h-*` additionally accept Tailwind's named container
/// scale (`max-w-md` = `--container-md` = 28rem), which the plain
/// width/height utilities don't.
fn parse_max_size_suffix(suffix: &str) -> Option<Dimension> {
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
        _ => return parse_dimension_suffix(suffix),
    };
    Some(Dimension::Length(Length::Px(rem * 16.0)))
}

/// Width/height accept more than the spacing scale: `w-1/2` fractions and
/// `w-full`/`w-auto` keywords (the latter handled by the exact-match table).
fn parse_dimension_suffix(suffix: &str) -> Option<Dimension> {
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
        "start" => Some(StyleProperty::InsetInlineStart(value)),
        "end" => Some(StyleProperty::InsetInlineEnd(value)),
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
    if let Some(rest) = token.strip_prefix("dark:") {
        return (Condition::Dark, rest);
    }
    if let Some(rest) = token.strip_prefix("first:") {
        return (Condition::FirstChild, rest);
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
            return vec![StyleProperty::MarginInlineStart(v), StyleProperty::MarginInlineEnd(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("my-") {
        if let Some(v) = parse_margin_suffix(rest) {
            return vec![StyleProperty::MarginTop(v), StyleProperty::MarginBottom(v)];
        }
    }
    if let Some(rest) = token.strip_prefix("m-") {
        if let Some(v) = parse_margin_suffix(rest) {
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
    if let Some(prop) = expand_mask(token) {
        return vec![prop];
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
            return vec![StyleProperty::Width(d), StyleProperty::Height(d)];
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
        // keeps leading-6, Dowel takes text-xl's 28px.
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
                vec![
                    StyleProperty::PaddingInlineStart(Length::Px(16.0)),
                    StyleProperty::PaddingInlineEnd(Length::Px(16.0))
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
        // Web backend can, and dowel_native raises a build error naming it.
        assert_eq!(
            expand_utility("grid"),
            (Condition::Always, vec![StyleProperty::Display(Display::Grid)])
        );
        assert!(!Display::Grid.is_supported_on_native());
        assert!(Display::None.is_supported_on_native());
    }

    #[test]
    fn parses_the_remaining_tier_one_utilities() {
        assert_eq!(
            expand_utility("z-10"),
            (Condition::Always, vec![StyleProperty::ZIndex(10)])
        );
        assert_eq!(
            expand_utility("min-w-0"),
            (Condition::Always, vec![StyleProperty::MinWidth(Dimension::Length(Length::Px(0.0)))])
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
    fn viewport_sizes_parse_and_are_flagged_web_only() {
        let (_, props) = expand_utility("h-screen");
        assert_eq!(props, vec![StyleProperty::Height(Dimension::ViewportHeight(100.0))]);
        assert!(props[0].unsupported_on_native().is_some());
        // A plain length on the same property stays portable.
        assert!(StyleProperty::Height(Dimension::Length(Length::Px(4.0)))
            .unsupported_on_native()
            .is_none());
    }

    #[test]
    fn parses_effects_and_transforms() {
        assert_eq!(
            expand_utility("blur-sm"),
            (Condition::Always, vec![StyleProperty::Filter("blur(8px)".to_string())])
        );
        assert_eq!(
            expand_utility("shadow-lg").1,
            vec![StyleProperty::BoxShadow(
                "0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1)".to_string()
            )]
        );
        assert_eq!(
            expand_utility("rotate-45"),
            (Condition::Always, vec![StyleProperty::Rotate(Angle { degrees: 45.0 })])
        );
        // Tailwind's scale scale is a percentage; the IR keeps the ratio.
        assert_eq!(
            expand_utility("scale-95"),
            (Condition::Always, vec![StyleProperty::Scale(95.0)])
        );
        assert_eq!(
            expand_utility("translate-x-2"),
            (Condition::Always, vec![StyleProperty::TranslateX(Length::Px(8.0))])
        );
    }

    #[test]
    fn negated_transforms_are_recognized() {
        assert_eq!(
            expand_utility("-rotate-45"),
            (Condition::Always, vec![StyleProperty::Rotate(Angle { degrees: -45.0 })])
        );
        assert_eq!(
            expand_utility("-translate-y-2"),
            (Condition::Always, vec![StyleProperty::TranslateY(Length::Px(-8.0))])
        );
    }

    #[test]
    fn direction_relative_utilities_stay_logical() {
        // These are the ones that actually flip between LTR and RTL, so
        // they must not be resolved to a physical side at compile time --
        // which side "start" is isn't known until runtime.
        assert_eq!(
            expand_utility("ps-4"),
            (Condition::Always, vec![StyleProperty::PaddingInlineStart(Length::Px(16.0))])
        );
        assert_eq!(
            expand_utility("me-2"),
            (Condition::Always, vec![StyleProperty::MarginInlineEnd(Dimension::Length(Length::Px(8.0)))])
        );
        assert_eq!(
            expand_utility("start-2"),
            (Condition::Always, vec![StyleProperty::InsetInlineStart(Length::Px(8.0))])
        );
        // The physical ones stay physical -- Tailwind has both families.
        assert_eq!(
            expand_utility("pl-4"),
            (Condition::Always, vec![StyleProperty::PaddingLeft(Length::Px(16.0))])
        );
        assert_eq!(
            expand_utility("left-2"),
            (Condition::Always, vec![StyleProperty::InsetLeft(Length::Px(8.0))])
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
        // The gradient half isn't implemented; it must stay unsupported
        // rather than be half-read as one of the above.
        assert!(expand_utility("mask-t-from-4").1.is_empty());
    }

    #[test]
    fn scroll_margin_and_padding_cover_every_edge() {
        assert_eq!(
            expand_utility("scroll-mt-4").1,
            vec![StyleProperty::ScrollMargin(Edge::Top, Length::Px(16.0))]
        );
        assert_eq!(
            expand_utility("scroll-pe-2").1,
            vec![StyleProperty::ScrollPadding(Edge::InlineEnd, Length::Px(8.0))]
        );
        assert_eq!(
            expand_utility("-scroll-mx-4").1,
            vec![StyleProperty::ScrollMargin(Edge::Inline, Length::Px(-16.0))]
        );
        // Padding takes no negative value, in CSS or in Tailwind.
        assert!(expand_utility("-scroll-p-4").1.is_empty());
        assert_eq!(expand_utility("scroll-smooth").1, vec![StyleProperty::ScrollBehaviorSmooth]);
        // `scroll-auto` is the initial value; left unsupported rather than
        // emitted as a no-op declaration.
        assert!(expand_utility("scroll-auto").1.is_empty());
    }

    #[test]
    fn per_side_border_colors_reach_the_right_side() {
        // Until 2026-08-15 every one of these compiled to `border-color`
        // -- all four sides -- from a token that isn't a colour name
        // (`var(--dowel-color-b-red-500)`). Wrong property, wrong value.
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
            vec![StyleProperty::InsetInline(Length::Px(16.0))]
        );
        assert_eq!(
            expand_utility("inset-bs-2").1,
            vec![StyleProperty::InsetBlockStart(Length::Px(8.0))]
        );
        assert_eq!(
            expand_utility("-inset-y-4").1,
            vec![StyleProperty::InsetBlock(Length::Px(-16.0))]
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
        // Dowel resolves this by source order (last wins), so `leading-*`
        // must be written after `text-*` to take effect. Real Tailwind is
        // order-independent here (it routes leading through a --tw-leading
        // custom property) -- a known, documented divergence.
        let (_, text_props) = expand_utility("text-xl");
        let (_, leading_props) = expand_utility("leading-6");
        let combined: Vec<_> = text_props.into_iter().chain(leading_props).collect();
        let deduped = dowel_ir::dedupe_last_wins(combined);
        assert!(deduped.contains(&StyleProperty::LineHeight(LineHeight::Length(Length::Px(24.0)))));
        assert!(!deduped.contains(&StyleProperty::LineHeight(LineHeight::Length(Length::Px(28.0)))));
    }

    #[test]
    fn named_leading_stays_a_ratio_rather_than_being_faked_as_pixels() {
        // The named scale is a unitless multiple of the element's own font
        // size. It's kept as a ratio -- which CSS states directly -- rather
        // than converted to a pixel value that would only be right for one
        // font size. React Native can't express it, and refuses it.
        let (_, props) = expand_utility("leading-tight");
        assert_eq!(props, vec![StyleProperty::LineHeight(LineHeight::Ratio(1.25))]);
        assert!(props[0].unsupported_on_native().is_some());

        // The numeric scale is spacing-based and resolves to a length on
        // both platforms.
        let (_, props) = expand_utility("leading-6");
        assert_eq!(props, vec![StyleProperty::LineHeight(LineHeight::Length(Length::Px(24.0)))]);
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
            (Condition::FirstChild, vec![StyleProperty::MarginTop(Dimension::Length(Length::Px(0.0)))])
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
            vec![StyleProperty::LetterSpacing(Em(0.025))]
        );
        assert_eq!(
            expand_utility("grid-cols-3").1,
            vec![StyleProperty::GridTemplateColumns(3)]
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
