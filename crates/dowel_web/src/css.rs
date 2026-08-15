//! `StyleProperty`/`Condition` -> CSS text.
//!
//! Grouping/flattening declarations by `Condition` lives in `dowel_ir`
//! (shared with `dowel_native`, which needs the identical rule -- "last
//! wins" only within one condition group). This module is just the
//! Web-specific value/selector formatting on top of that.
//!
//! `Color` stays a Tailwind token through the whole IR (proposal §16 still
//! defers *theme-aware* resolution to a later pass -- custom colors,
//! arbitrary values), but the default Tailwind palette is resolved here via
//! `dowel_ir::resolve_color_token`, emitted as the exact `oklch(...)`
//! string Tailwind's own CSS would produce. A token outside the default
//! palette still falls back to a CSS custom property reference
//! (`var(--dowel-color-x)`, never actually defined anywhere) --
//! correct-but-unresolved, not silently wrong.

use dowel_ir::{
    Align, AlignSelf, BorderStyle, Breakpoint, Color, Condition, ConditionExpr, DecorationStyle,
    Dimension, Display, Edge, MaskSlot, MaskStop,
    Em, FlexDirection, FlexShorthand, Justify, Length, LineHeight, Overflow, Position, Radius,
    StyleProperty, TextAlign, TextOverflow, TextTransform, WhiteSpace,
};

fn length_px(length: Length) -> String {
    let Length::Px(value) = length;
    format!("{value}px")
}

fn dimension_value(dim: Dimension) -> String {
    match dim {
        Dimension::Length(length) => length_px(length),
        Dimension::Percent(pct) => format!("{pct}%"),
        Dimension::Auto => "auto".to_string(),
        Dimension::ViewportWidth(pct) => format!("{pct}vw"),
        Dimension::ViewportHeight(pct) => format!("{pct}vh"),
    }
}

fn justify_keyword(justify: &Justify) -> &'static str {
    match justify {
        Justify::Start => "flex-start",
        Justify::Center => "center",
        Justify::End => "flex-end",
        Justify::Between => "space-between",
        Justify::Around => "space-around",
        Justify::Evenly => "space-evenly",
    }
}

fn border_style_keyword(style: &BorderStyle) -> &'static str {
    match style {
        BorderStyle::Solid => "solid",
        BorderStyle::Dashed => "dashed",
        BorderStyle::Dotted => "dotted",
        BorderStyle::Double => "double",
        BorderStyle::Hidden => "hidden",
        BorderStyle::None => "none",
    }
}

fn radius_value(radius: &Radius) -> String {
    match radius {
        Radius::Length(l) => length_px(*l),
        // Exactly what Tailwind emits -- CSS can state this, so there's no
        // reason to approximate it here.
        Radius::Full => "calc(infinity * 1px)".to_string(),
    }
}

/// The CSS longhand for each edge. Spelled out rather than concatenated
/// because `property_and_value` returns `&'static str`, and a built string
/// would have to be leaked to satisfy that.
fn scroll_margin_property(edge: Edge) -> &'static str {
    match edge {
        Edge::All => "scroll-margin",
        Edge::Top => "scroll-margin-top",
        Edge::Right => "scroll-margin-right",
        Edge::Bottom => "scroll-margin-bottom",
        Edge::Left => "scroll-margin-left",
        Edge::Inline => "scroll-margin-inline",
        Edge::Block => "scroll-margin-block",
        Edge::InlineStart => "scroll-margin-inline-start",
        Edge::InlineEnd => "scroll-margin-inline-end",
        Edge::BlockStart => "scroll-margin-block-start",
        Edge::BlockEnd => "scroll-margin-block-end",
    }
}

fn scroll_padding_property(edge: Edge) -> &'static str {
    match edge {
        Edge::All => "scroll-padding",
        Edge::Top => "scroll-padding-top",
        Edge::Right => "scroll-padding-right",
        Edge::Bottom => "scroll-padding-bottom",
        Edge::Left => "scroll-padding-left",
        Edge::Inline => "scroll-padding-inline",
        Edge::Block => "scroll-padding-block",
        Edge::InlineStart => "scroll-padding-inline-start",
        Edge::InlineEnd => "scroll-padding-inline-end",
        Edge::BlockStart => "scroll-padding-block-start",
        Edge::BlockEnd => "scroll-padding-block-end",
    }
}

/// Tailwind's unset-slot filler: opaque, so `mask-composite: intersect`
/// leaves whatever the other slots paint untouched.
const MASK_OPAQUE: &str = "linear-gradient(#fff, #fff)";

fn is_mask_gradient(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::MaskStopColor(..)
            | StyleProperty::MaskStopPosition(..)
            | StyleProperty::MaskAngle(..)
            | StyleProperty::MaskRadialShape(_)
            | StyleProperty::MaskRadialSize(_)
            | StyleProperty::MaskRadialPosition(_)
            | StyleProperty::MaskComposite(_)
    )
}

/// One slot's stops, or `None` if no utility touched it.
struct MaskGradient {
    from_color: Option<String>,
    from_position: Option<String>,
    to_color: Option<String>,
    to_position: Option<String>,
    angle: Option<f64>,
}

impl MaskGradient {
    /// Whether any utility contributed to this slot at all.
    fn is_set(&self) -> bool {
        self.from_color.is_some()
            || self.from_position.is_some()
            || self.to_color.is_some()
            || self.to_position.is_some()
            || self.angle.is_some()
    }

    /// Whether a stop list should be written. An angle alone produces
    /// `linear-gradient(45deg)` with no stops, matching Tailwind's
    /// `var(--tw-mask-linear-stops, var(--tw-mask-linear-position))`
    /// fallback.
    fn has_stops(&self) -> bool {
        self.from_color.is_some()
            || self.from_position.is_some()
            || self.to_color.is_some()
            || self.to_position.is_some()
    }

    /// The `<from> <pos>, <to> <pos>` half, with Tailwind's register
    /// defaults filled in.
    fn stops(&self) -> String {
        format!(
            "{} {}, {} {}",
            self.from_color.as_deref().unwrap_or("black"),
            self.from_position.as_deref().unwrap_or("0%"),
            self.to_color.as_deref().unwrap_or("transparent"),
            self.to_position.as_deref().unwrap_or("100%"),
        )
    }
}

/// Resolves the whole `mask-image` layer list at compile time.
///
/// Tailwind assembles it from `--tw-mask-*` registers, so the same
/// `mask-image: var(--tw-mask-linear), var(--tw-mask-radial),
/// var(--tw-mask-conic)` appears on every gradient utility and the
/// difference lives in which registers each one sets. Dowel has the whole
/// set in hand, so it writes the resolved list and ships no custom
/// properties.
///
/// The first slot is overloaded in Tailwind too: a side utility makes it a
/// four-layer list (left, right, bottom, top), a `mask-linear-*` makes it a
/// single gradient. Using both kinds together is therefore
/// order-dependent in Tailwind and resolved here as "sides win", which is
/// the only case where the two can disagree.
fn mask_declarations(props: &[&StyleProperty]) -> Vec<(&'static str, String)> {
    let slot_gradient = |slot: MaskSlot| {
        let mut g = MaskGradient {
            from_color: None,
            from_position: None,
            to_color: None,
            to_position: None,
            angle: None,
        };
        for prop in props {
            match prop {
                StyleProperty::MaskStopColor(s, stop, c) if *s == slot => match stop {
                    MaskStop::From => g.from_color = Some(color_var(c)),
                    MaskStop::To => g.to_color = Some(color_var(c)),
                },
                StyleProperty::MaskStopPosition(s, stop, d) if *s == slot => match stop {
                    MaskStop::From => g.from_position = Some(dimension_value(*d)),
                    MaskStop::To => g.to_position = Some(dimension_value(*d)),
                },
                StyleProperty::MaskAngle(s, degrees) if *s == slot => g.angle = Some(*degrees),
                _ => {}
            }
        }
        g
    };
    let keyword = |find: fn(&StyleProperty) -> Option<&'static str>, default: &'static str| {
        props.iter().find_map(|p| find(p)).unwrap_or(default)
    };

    let sides = [MaskSlot::Left, MaskSlot::Right, MaskSlot::Bottom, MaskSlot::Top];
    let side_gradients: Vec<(MaskSlot, MaskGradient)> =
        sides.iter().map(|s| (*s, slot_gradient(*s))).collect();
    let any_side = side_gradients.iter().any(|(_, g)| g.is_set());

    let linear = slot_gradient(MaskSlot::Linear);
    let radial = slot_gradient(MaskSlot::Radial);
    let conic = slot_gradient(MaskSlot::Conic);

    let composite = props.iter().find_map(|p| match p {
        StyleProperty::MaskComposite(c) => Some(*c),
        _ => None,
    });

    let paints = any_side || linear.is_set() || radial.is_set() || conic.is_set();
    if !paints {
        // `mask-add` on its own, or only radial shaping -- Tailwind emits
        // the composite alone and no `mask-image`.
        return composite.map_or_else(Vec::new, |c| vec![("mask-composite", c.to_string())]);
    }

    let mut layers: Vec<String> = Vec::new();
    if any_side {
        for (slot, g) in &side_gradients {
            layers.push(if g.is_set() {
                format!("linear-gradient(to {}, {})", side_keyword(*slot), g.stops())
            } else {
                MASK_OPAQUE.to_string()
            });
        }
    } else {
        layers.push(match (linear.is_set(), linear.has_stops()) {
            (false, _) => MASK_OPAQUE.to_string(),
            (true, false) => format!("linear-gradient({}deg)", linear.angle.unwrap_or(0.0)),
            (true, true) => {
                format!("linear-gradient({}deg, {})", linear.angle.unwrap_or(0.0), linear.stops())
            }
        });
    }

    layers.push(if radial.is_set() {
        format!(
            "radial-gradient({} {} at {}, {})",
            keyword(
                |p| match p {
                    StyleProperty::MaskRadialShape(v) => Some(*v),
                    _ => None,
                },
                "ellipse"
            ),
            keyword(
                |p| match p {
                    StyleProperty::MaskRadialSize(v) => Some(*v),
                    _ => None,
                },
                "farthest-corner"
            ),
            keyword(
                |p| match p {
                    StyleProperty::MaskRadialPosition(v) => Some(*v),
                    _ => None,
                },
                "center"
            ),
            radial.stops(),
        )
    } else {
        MASK_OPAQUE.to_string()
    });

    layers.push(match (conic.is_set(), conic.has_stops()) {
        (false, _) => MASK_OPAQUE.to_string(),
        (true, false) => format!("conic-gradient({}deg)", conic.angle.unwrap_or(0.0)),
        (true, true) => {
            format!("conic-gradient(from {}deg, {})", conic.angle.unwrap_or(0.0), conic.stops())
        }
    });

    vec![
        ("mask-image", layers.join(", ")),
        ("mask-composite", composite.unwrap_or("intersect").to_string()),
    ]
}

fn is_border_spacing(prop: &StyleProperty) -> bool {
    matches!(prop, StyleProperty::BorderSpacingX(_) | StyleProperty::BorderSpacingY(_))
}

fn is_translate(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::TranslateX(_) | StyleProperty::TranslateY(_) | StyleProperty::TranslateZ(_)
    )
}

/// CSS's `translate` is one property taking up to three values, so the
/// three axes have to become one declaration. Emitting them separately --
/// which this did until 2026-08-15 -- made `translate-x-4 translate-y-8`
/// write two `translate:` declarations, and last-wins threw the x away.
///
/// The z value is only written when present, matching Tailwind: the
/// two-value form is what `translate-x-*`/`translate-y-*` produce.
fn translate_value(props: &[&StyleProperty]) -> Option<String> {
    if props.is_empty() {
        return None;
    }
    let axis = |f: fn(&StyleProperty) -> Option<String>| {
        props.iter().find_map(|p| f(p)).unwrap_or_else(|| "0".to_string())
    };
    let x = axis(|p| match p {
        StyleProperty::TranslateX(d) => Some(dimension_value(*d)),
        _ => None,
    });
    let y = axis(|p| match p {
        StyleProperty::TranslateY(d) => Some(dimension_value(*d)),
        _ => None,
    });
    let z = props.iter().find_map(|p| match p {
        StyleProperty::TranslateZ(l) => Some(length_px(*l)),
        _ => None,
    });
    Some(match z {
        Some(z) => format!("{x} {y} {z}"),
        None => format!("{x} {y}"),
    })
}

/// `border-spacing` takes a horizontal and a vertical value in one
/// declaration, so the two axes compose. An unset axis is `0`, matching
/// what Tailwind writes for `border-spacing-x-*`.
fn border_spacing_value(props: &[&StyleProperty]) -> Option<String> {
    if props.is_empty() {
        return None;
    }
    let axis = |f: fn(&StyleProperty) -> Option<Length>| {
        props.iter().find_map(|p| f(p)).map(length_px).unwrap_or_else(|| "0".to_string())
    };
    Some(format!(
        "{} {}",
        axis(|p| match p {
            StyleProperty::BorderSpacingX(l) => Some(*l),
            _ => None,
        }),
        axis(|p| match p {
            StyleProperty::BorderSpacingY(l) => Some(*l),
            _ => None,
        }),
    ))
}

fn is_scrollbar_color(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::ScrollbarThumbColor(_) | StyleProperty::ScrollbarTrackColor(_)
    )
}

/// `scrollbar-color` takes both halves at once, so `scrollbar-thumb-*` and
/// `scrollbar-track-*` compose into one declaration. Tailwind's registers
/// default to `#0000`, so an unset half is transparent rather than the UA
/// default -- which is why writing only one still names both.
fn scrollbar_color_value(props: &[&StyleProperty]) -> Option<String> {
    let find = |f: fn(&StyleProperty) -> Option<&Color>| {
        props.iter().find_map(|p| f(p)).map(color_var).unwrap_or_else(|| "#0000".to_string())
    };
    if props.is_empty() {
        return None;
    }
    Some(format!(
        "{} {}",
        find(|p| match p {
            StyleProperty::ScrollbarThumbColor(c) => Some(c),
            _ => None,
        }),
        find(|p| match p {
            StyleProperty::ScrollbarTrackColor(c) => Some(c),
            _ => None,
        }),
    ))
}

fn side_keyword(slot: MaskSlot) -> &'static str {
    match slot {
        MaskSlot::Left => "left",
        MaskSlot::Right => "right",
        MaskSlot::Bottom => "bottom",
        MaskSlot::Top => "top",
        _ => "top",
    }
}

fn color_var(color: &Color) -> String {
    let Color::Token(token) = color;
    match dowel_ir::resolve_color_token(token) {
        Some(resolved) => resolved.oklch.to_string(),
        None => format!("var(--dowel-color-{token})"),
    }
}

/// Maps one `StyleProperty` to a `(css-property-name, value)` pair. Values
/// mirror Tailwind's own generated CSS where there's a choice (e.g.
/// `align-items: flex-start` rather than the newer `start` keyword) so
/// output stays recognizable to anyone used to reading Tailwind's CSS.
pub fn property_and_value(prop: &StyleProperty) -> (&'static str, String) {
    match prop {
        StyleProperty::Display(d) => (
            "display",
            match d {
                Display::Flex => "flex",
                Display::None => "none",
                Display::Contents => "contents",
                Display::Block => "block",
                Display::InlineFlex => "inline-flex",
                Display::Grid => "grid",
            }
            .to_string(),
        ),
        StyleProperty::FlexDirection(dir) => (
            "flex-direction",
            match dir {
                FlexDirection::Row => "row",
                FlexDirection::Column => "column",
                FlexDirection::RowReverse => "row-reverse",
                FlexDirection::ColumnReverse => "column-reverse",
            }
            .to_string(),
        ),
        StyleProperty::Flex(shorthand) => (
            "flex",
            match shorthand {
                FlexShorthand::Grow(n) => format!("{n} 1 0%"),
                FlexShorthand::Auto => "1 1 auto".to_string(),
                FlexShorthand::Initial => "0 1 auto".to_string(),
                FlexShorthand::None => "none".to_string(),
            },
        ),
        StyleProperty::AlignItems(align) => (
            "align-items",
            match align {
                Align::Start => "flex-start",
                Align::Center => "center",
                Align::End => "flex-end",
                Align::Stretch => "stretch",
                Align::Baseline => "baseline",
            }
            .to_string(),
        ),
        StyleProperty::AlignSelf(align) => (
            "align-self",
            match align {
                AlignSelf::Auto => "auto",
                AlignSelf::Start => "flex-start",
                AlignSelf::Center => "center",
                AlignSelf::End => "flex-end",
                AlignSelf::Stretch => "stretch",
                AlignSelf::Baseline => "baseline",
            }
            .to_string(),
        ),
        StyleProperty::AlignContent(justify) => ("align-content", justify_keyword(justify).to_string()),
        StyleProperty::JustifyContent(justify) => {
            ("justify-content", justify_keyword(justify).to_string())
        }
        StyleProperty::Gap(l) => ("gap", length_px(*l)),
        StyleProperty::RowGap(l) => ("row-gap", length_px(*l)),
        StyleProperty::ColumnGap(l) => ("column-gap", length_px(*l)),
        StyleProperty::MarginTop(d) => ("margin-top", dimension_value(*d)),
        StyleProperty::MarginRight(d) => ("margin-right", dimension_value(*d)),
        StyleProperty::MarginBottom(d) => ("margin-bottom", dimension_value(*d)),
        StyleProperty::MarginLeft(d) => ("margin-left", dimension_value(*d)),
        StyleProperty::PaddingTop(l) => ("padding-top", length_px(*l)),
        StyleProperty::PaddingRight(l) => ("padding-right", length_px(*l)),
        StyleProperty::PaddingBottom(l) => ("padding-bottom", length_px(*l)),
        StyleProperty::PaddingLeft(l) => ("padding-left", length_px(*l)),
        StyleProperty::MarginInlineStart(d) => ("margin-inline-start", dimension_value(*d)),
        StyleProperty::MarginInlineEnd(d) => ("margin-inline-end", dimension_value(*d)),
        StyleProperty::PaddingInlineStart(l) => ("padding-inline-start", length_px(*l)),
        StyleProperty::PaddingInlineEnd(l) => ("padding-inline-end", length_px(*l)),
        StyleProperty::Width(d) => ("width", dimension_value(*d)),
        StyleProperty::Height(d) => ("height", dimension_value(*d)),
        StyleProperty::MinWidth(d) => ("min-width", dimension_value(*d)),
        StyleProperty::MinHeight(d) => ("min-height", dimension_value(*d)),
        StyleProperty::MaxWidth(d) => ("max-width", dimension_value(*d)),
        StyleProperty::MaxHeight(d) => ("max-height", dimension_value(*d)),
        StyleProperty::ZIndex(z) => ("z-index", format!("{z}")),
        StyleProperty::GridTemplateColumns(n) => {
            ("grid-template-columns", format!("repeat({n}, minmax(0, 1fr))"))
        }
        StyleProperty::Position(pos) => (
            "position",
            match pos {
                Position::Relative => "relative",
                Position::Absolute => "absolute",
            }
            .to_string(),
        ),
        StyleProperty::InsetTop(d) => ("top", dimension_value(*d)),
        StyleProperty::InsetRight(d) => ("right", dimension_value(*d)),
        StyleProperty::InsetBottom(d) => ("bottom", dimension_value(*d)),
        StyleProperty::InsetLeft(d) => ("left", dimension_value(*d)),
        StyleProperty::InsetInlineStart(d) => ("inset-inline-start", dimension_value(*d)),
        StyleProperty::InsetInlineEnd(d) => ("inset-inline-end", dimension_value(*d)),
        StyleProperty::InsetInline(d) => ("inset-inline", dimension_value(*d)),
        StyleProperty::InsetBlock(d) => ("inset-block", dimension_value(*d)),
        StyleProperty::InsetBlockStart(d) => ("inset-block-start", dimension_value(*d)),
        StyleProperty::InsetBlockEnd(d) => ("inset-block-end", dimension_value(*d)),
        StyleProperty::BackgroundColor(c) => ("background-color", color_var(c)),
        StyleProperty::Opacity(o) => ("opacity", format!("{o}")),
        StyleProperty::BorderColor(c) => ("border-color", color_var(c)),
        StyleProperty::ScrollMargin(edge, l) => (scroll_margin_property(*edge), length_px(*l)),
        StyleProperty::ScrollPadding(edge, l) => (scroll_padding_property(*edge), length_px(*l)),
        StyleProperty::ScrollBehaviorSmooth => ("scroll-behavior", "smooth".to_string()),
        StyleProperty::MaskClip(v) => ("mask-clip", v.to_string()),
        StyleProperty::MaskOrigin(v) => ("mask-origin", v.to_string()),
        StyleProperty::MaskMode(v) => ("mask-mode", v.to_string()),
        StyleProperty::MaskType(v) => ("mask-type", v.to_string()),
        StyleProperty::MaskSize(v) => ("mask-size", v.to_string()),
        StyleProperty::MaskPosition(v) => ("mask-position", v.to_string()),
        StyleProperty::MaskRepeat(v) => ("mask-repeat", v.to_string()),
        StyleProperty::MaskImageNone => ("mask-image", "none".to_string()),
        StyleProperty::ScrollbarWidth(v) => ("scrollbar-width", v.to_string()),
        StyleProperty::ScrollbarGutter(v) => ("scrollbar-gutter", v.to_string()),
        // Composed by `scrollbar_color_value`; partitioned out above.
        StyleProperty::ScrollbarThumbColor(_) | StyleProperty::ScrollbarTrackColor(_) => {
            ("scrollbar-color", String::new())
        }
        // Composed by `mask_declarations`; `render_rule` partitions these
        // out before this runs.
        StyleProperty::MaskStopColor(..)
        | StyleProperty::MaskStopPosition(..)
        | StyleProperty::MaskAngle(..)
        | StyleProperty::MaskRadialShape(_)
        | StyleProperty::MaskRadialSize(_)
        | StyleProperty::MaskRadialPosition(_)
        | StyleProperty::MaskComposite(_) => ("mask-image", String::new()),
        StyleProperty::Fill(c) => ("fill", color_var(c)),
        StyleProperty::Stroke(c) => ("stroke", color_var(c)),
        // SVG stroke-width is unitless, unlike every other length here.
        StyleProperty::StrokeWidth(n) => ("stroke-width", format!("{n}")),
        StyleProperty::AccentColor(c) => ("accent-color", color_var(c)),
        StyleProperty::CaretColor(c) => ("caret-color", color_var(c)),
        StyleProperty::TextDecorationColor(c) => ("text-decoration-color", color_var(c)),
        StyleProperty::TextDecorationStyle(s) => (
            "text-decoration-style",
            match s {
                DecorationStyle::Solid => "solid",
                DecorationStyle::Double => "double",
                DecorationStyle::Dotted => "dotted",
                DecorationStyle::Dashed => "dashed",
                DecorationStyle::Wavy => "wavy",
            }
            .to_string(),
        ),
        StyleProperty::TextDecorationThickness(l) => ("text-decoration-thickness", length_px(*l)),
        // Emitted into its own `::placeholder` rule by `render_rule`.
        StyleProperty::PlaceholderColor(c) => ("color", color_var(c)),
        StyleProperty::OutlineWidth(l) => ("outline-width", length_px(*l)),
        StyleProperty::OutlineStyle(s) => ("outline-style", border_style_keyword(s).to_string()),
        StyleProperty::OutlineColor(c) => ("outline-color", color_var(c)),
        StyleProperty::OutlineOffset(l) => ("outline-offset", length_px(*l)),
        // Child-scoped; `render_rule` partitions these into their own rule
        // before this runs (see `space_declarations`).
        StyleProperty::DivideX(_)
        | StyleProperty::DivideY(_)
        | StyleProperty::DivideColor(_)
        | StyleProperty::DivideStyle(_) => ("border-color", String::new()),
        // One CSS longhand each, including the two axis shorthands, which
        // is exactly what Tailwind emits.
        StyleProperty::BorderTopColor(c) => ("border-top-color", color_var(c)),
        StyleProperty::BorderRightColor(c) => ("border-right-color", color_var(c)),
        StyleProperty::BorderBottomColor(c) => ("border-bottom-color", color_var(c)),
        StyleProperty::BorderLeftColor(c) => ("border-left-color", color_var(c)),
        StyleProperty::BorderInlineColor(c) => ("border-inline-color", color_var(c)),
        StyleProperty::BorderBlockColor(c) => ("border-block-color", color_var(c)),
        StyleProperty::BorderInlineStartColor(c) => ("border-inline-start-color", color_var(c)),
        StyleProperty::BorderInlineEndColor(c) => ("border-inline-end-color", color_var(c)),
        StyleProperty::BorderBlockStartColor(c) => ("border-block-start-color", color_var(c)),
        StyleProperty::BorderBlockEndColor(c) => ("border-block-end-color", color_var(c)),
        StyleProperty::BorderTopWidth(l) => ("border-top-width", length_px(*l)),
        StyleProperty::BorderRightWidth(l) => ("border-right-width", length_px(*l)),
        StyleProperty::BorderBottomWidth(l) => ("border-bottom-width", length_px(*l)),
        StyleProperty::BorderLeftWidth(l) => ("border-left-width", length_px(*l)),
        StyleProperty::BorderTopStyle(s) => ("border-top-style", border_style_keyword(s).to_string()),
        StyleProperty::BorderRightStyle(s) => {
            ("border-right-style", border_style_keyword(s).to_string())
        }
        StyleProperty::BorderBottomStyle(s) => {
            ("border-bottom-style", border_style_keyword(s).to_string())
        }
        StyleProperty::BorderLeftStyle(s) => ("border-left-style", border_style_keyword(s).to_string()),
        StyleProperty::BorderRadius(r) => ("border-radius", radius_value(r)),
        StyleProperty::BorderTopLeftRadius(r) => ("border-top-left-radius", radius_value(r)),
        StyleProperty::BorderTopRightRadius(r) => ("border-top-right-radius", radius_value(r)),
        StyleProperty::BorderBottomRightRadius(r) => ("border-bottom-right-radius", radius_value(r)),
        StyleProperty::BorderBottomLeftRadius(r) => ("border-bottom-left-radius", radius_value(r)),
        StyleProperty::BorderStartStartRadius(r) => ("border-start-start-radius", radius_value(r)),
        StyleProperty::BorderStartEndRadius(r) => ("border-start-end-radius", radius_value(r)),
        StyleProperty::BorderEndStartRadius(r) => ("border-end-start-radius", radius_value(r)),
        StyleProperty::BorderEndEndRadius(r) => ("border-end-end-radius", radius_value(r)),
        StyleProperty::FontSize(l) => ("font-size", length_px(*l)),
        StyleProperty::FontWeight(w) => ("font-weight", format!("{}", w.0)),
        StyleProperty::LineHeight(lh) => (
            "line-height",
            match lh {
                LineHeight::Length(l) => length_px(*l),
                LineHeight::Ratio(r) => format!("{r}"),
            },
        ),
        StyleProperty::LetterSpacing(Em(v)) => ("letter-spacing", format!("{v}em")),
        StyleProperty::Overflow(o) => (
            "overflow",
            match o {
                Overflow::Visible => "visible",
                Overflow::Hidden => "hidden",
                Overflow::Scroll => "scroll",
            }
            .to_string(),
        ),
        StyleProperty::TextOverflow(t) => (
            "text-overflow",
            match t {
                TextOverflow::Clip => "clip",
                TextOverflow::Ellipsis => "ellipsis",
            }
            .to_string(),
        ),
        StyleProperty::WhiteSpace(w) => (
            "white-space",
            match w {
                WhiteSpace::Normal => "normal",
                WhiteSpace::NoWrap => "nowrap",
            }
            .to_string(),
        ),
        StyleProperty::TransitionProperty(p) => ("transition-property", p.clone()),
        StyleProperty::TransitionDuration(ms) => ("transition-duration", format!("{ms}ms")),
        StyleProperty::TransitionTimingFunction(f) => ("transition-timing-function", f.clone()),
        StyleProperty::Animation(a) => ("animation", a.shorthand().to_string()),
        // Never reached: `render_rule` partitions these out into their own
        // child-scoped rule before calling this. Emitting the margin on the
        // element itself would be wrong, so there's nothing sensible to
        // return -- an empty name is filtered by the caller.
        StyleProperty::SpaceX(_) | StyleProperty::SpaceY(_) => ("", String::new()),
        StyleProperty::TextAlign(align) => (
            "text-align",
            match align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Right => "right",
            }
            .to_string(),
        ),
        // Standalone properties, as CSS defines them and Tailwind emits
        // them -- the `transform` shorthand isn't used on either side.
        // Tailwind writes both axes explicitly for scale/translate, so
        // these do the same rather than relying on one-value expansion.
        StyleProperty::Rotate(a) => ("rotate", format!("{}deg", a.degrees)),
        StyleProperty::Scale(pct) => ("scale", format!("{pct}% {pct}%")),
        // Composed by `translate_value`; partitioned out above.
        StyleProperty::TranslateX(_) | StyleProperty::TranslateY(_) | StyleProperty::TranslateZ(_) => {
            ("translate", String::new())
        }
        StyleProperty::FlexBasis(d) => ("flex-basis", dimension_value(*d)),
        StyleProperty::BlockSize(d) => ("block-size", dimension_value(*d)),
        StyleProperty::InlineSize(d) => ("inline-size", dimension_value(*d)),
        StyleProperty::MaxBlockSize(d) => ("max-block-size", dimension_value(*d)),
        StyleProperty::MaxInlineSize(d) => ("max-inline-size", dimension_value(*d)),
        StyleProperty::MinBlockSize(d) => ("min-block-size", dimension_value(*d)),
        StyleProperty::MinInlineSize(d) => ("min-inline-size", dimension_value(*d)),
        StyleProperty::TextIndent(d) => ("text-indent", dimension_value(*d)),
        StyleProperty::MarginBlockStart(d) => ("margin-block-start", dimension_value(*d)),
        StyleProperty::MarginBlockEnd(d) => ("margin-block-end", dimension_value(*d)),
        StyleProperty::PaddingBlockStart(l) => ("padding-block-start", length_px(*l)),
        StyleProperty::PaddingBlockEnd(l) => ("padding-block-end", length_px(*l)),
        // `border-spacing` takes both axes at once, so these compose.
        StyleProperty::BorderSpacingX(_) | StyleProperty::BorderSpacingY(_) => {
            ("border-spacing", String::new())
        }
        // Composed with any ring layers by `box_shadow_value`, not emitted
        // here -- `render_rule` partitions these out before this runs.
        StyleProperty::BoxShadow(s) => ("box-shadow", s.clone()),
        StyleProperty::RingWidth(_)
        | StyleProperty::RingColor(_)
        | StyleProperty::InsetRingWidth(_)
        | StyleProperty::InsetRingColor(_) => ("box-shadow", String::new()),
        StyleProperty::Filter(f) => ("filter", f.clone()),
        StyleProperty::TextTransform(t) => (
            "text-transform",
            match t {
                TextTransform::Uppercase => "uppercase",
                TextTransform::Lowercase => "lowercase",
                TextTransform::Capitalize => "capitalize",
                TextTransform::None => "none",
            }
            .to_string(),
        ),
        StyleProperty::TextColor(c) => ("color", color_var(c)),
    }
}

fn breakpoint_min_width_px(bp: Breakpoint) -> u32 {
    match bp {
        Breakpoint::Sm => 640,
        Breakpoint::Md => 768,
        Breakpoint::Lg => 1024,
        Breakpoint::Xl => 1280,
        Breakpoint::Xl2 => 1536,
    }
}

/// A guard's CSS attribute-selector name, keyed by the source span of the
/// opaque expression it wraps -- two `ConditionExpr::Ref`s pointing at the
/// same span refer to the same runtime value, so they must resolve to the
/// same attribute name.
pub fn expr_ref_attribute(expr_ref: dowel_ir::ExprRef) -> String {
    format!("data-dowel-cond-{}-{}", expr_ref.0.start, expr_ref.0.end)
}

fn condition_expr_selector(expr: &ConditionExpr) -> String {
    match expr {
        ConditionExpr::Ref(expr_ref) => format!("[{}]", expr_ref_attribute(*expr_ref)),
        ConditionExpr::Not(inner) => format!(":not({})", condition_expr_selector(inner)),
        ConditionExpr::And(a, b) => format!("{}{}", condition_expr_selector(a), condition_expr_selector(b)),
        ConditionExpr::Or(a, b) => {
            format!(":is({}, {})", condition_expr_selector(a), condition_expr_selector(b))
        }
    }
}

/// A condition's shape as `(media query, selector suffix)` -- suffix is
/// appended directly after the node's own class in the compound selector
/// (e.g. `.dowel-0:hover`, `.dowel-0[data-dowel-cond-4-10]`).
pub fn condition_shape(condition: &Condition) -> (Option<String>, String) {
    match condition {
        Condition::Always => (None, String::new()),
        Condition::Hover => (None, ":hover".to_string()),
        Condition::Focus => (None, ":focus".to_string()),
        // Only meaningful on elements that can actually be disabled (e.g.
        // <button>) -- CSS itself won't apply `:disabled` to a plain <div>.
        Condition::Disabled => (None, ":disabled".to_string()),
        // Known gotcha, not fixed here: iOS Safari doesn't reliably fire
        // `:active` from a tap unless the element has some touch-event
        // listener attached (a long-documented WebKit quirk). Dowel's
        // compiled onClick doesn't count. Fine for the common desktop/
        // Android case; tracked as a real gap, not silently "handled."
        Condition::Pressed => (None, ":active".to_string()),
        Condition::Responsive(bp) => {
            (Some(format!("(min-width: {}px)", breakpoint_min_width_px(*bp))), String::new())
        }
        // Tailwind v4's default dark strategy, and the one whose meaning
        // React Native's `useColorScheme()` shares.
        Condition::Dark => (Some("(prefers-color-scheme: dark)".to_string()), String::new()),
        Condition::FirstChild => (None, ":first-child".to_string()),
        Condition::Expr(expr) => (None, condition_expr_selector(expr)),
    }
}

/// Renders one CSS rule (optionally media-wrapped) for a class + condition
/// group's already-deduped properties.
fn is_shadow_layer(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::BoxShadow(_)
            | StyleProperty::RingWidth(_)
            | StyleProperty::RingColor(_)
            | StyleProperty::InsetRingWidth(_)
            | StyleProperty::InsetRingColor(_)
    )
}

/// Joins whichever ring/shadow utilities are present into one `box-shadow`.
///
/// Tailwind does this at runtime with `--tw-*` registers spliced into a
/// fixed layer list; Dowel knows the whole set at compile time, so it
/// writes the resolved list directly and ships no custom properties. Layer
/// order follows Tailwind's: inset ring, then ring, then the shadow.
///
/// A ring colour with no width contributes nothing, which is correct --
/// `ring-blue-500` alone has nothing to paint, exactly as in Tailwind.
fn box_shadow_value(props: &[&StyleProperty]) -> Option<String> {
    let find_length = |f: fn(&StyleProperty) -> Option<Length>| props.iter().find_map(|p| f(p));
    let find_color = |f: fn(&StyleProperty) -> Option<&Color>| props.iter().find_map(|p| f(p));

    let ring = find_length(|p| match p {
        StyleProperty::RingWidth(l) => Some(*l),
        _ => None,
    });
    let inset_ring = find_length(|p| match p {
        StyleProperty::InsetRingWidth(l) => Some(*l),
        _ => None,
    });
    let ring_color = find_color(|p| match p {
        StyleProperty::RingColor(c) => Some(c),
        _ => None,
    });
    let inset_ring_color = find_color(|p| match p {
        StyleProperty::InsetRingColor(c) => Some(c),
        _ => None,
    });
    let shadow = props.iter().find_map(|p| match p {
        StyleProperty::BoxShadow(s) => Some(s.clone()),
        _ => None,
    });

    // Tailwind's default ring colour is `currentcolor`.
    let paint = |c: Option<&Color>| c.map_or("currentcolor".to_string(), color_var);

    let mut layers: Vec<String> = Vec::new();
    if let Some(width) = inset_ring {
        layers.push(format!("inset 0 0 0 {} {}", length_px(width), paint(inset_ring_color)));
    }
    if let Some(width) = ring {
        layers.push(format!("0 0 0 {} {}", length_px(width), paint(ring_color)));
    }
    if let Some(shadow) = shadow {
        // `shadow-none` removes the *shadow* layer, not the whole
        // declaration -- `shadow-none ring-2` still draws the ring, which is
        // what Tailwind does by clearing only its `--tw-shadow` register.
        if shadow != "none" {
            layers.push(shadow);
        } else if layers.is_empty() {
            return Some("none".to_string());
        }
    }
    (!layers.is_empty()).then(|| layers.join(", "))
}

pub fn render_rule(class_name: &str, condition: &Condition, props: &[StyleProperty]) -> String {
    let (media, suffix) = condition_shape(condition);

    // Some utilities target something other than the element itself, so
    // they become their own rule with a different selector rather than a
    // declaration here. `space-*`/`divide-*` reach the children;
    // `placeholder-*` reaches the `::placeholder` pseudo-element.
    let (scoped_props, own_props): (Vec<_>, Vec<_>) = props.iter().partition(|p| {
        matches!(
            p,
            StyleProperty::SpaceX(_)
                | StyleProperty::SpaceY(_)
                | StyleProperty::DivideX(_)
                | StyleProperty::DivideY(_)
                | StyleProperty::DivideColor(_)
                | StyleProperty::DivideStyle(_)
                | StyleProperty::PlaceholderColor(_)
        )
    });
    let (placeholder_props, child_props): (Vec<_>, Vec<_>) = scoped_props
        .iter()
        .partition(|p| matches!(p, StyleProperty::PlaceholderColor(_)));

    // Rings and shadows are several utilities that share one CSS property,
    // so they're composed rather than emitted one declaration each.
    let (shadow_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        own_props.into_iter().partition(|p| is_shadow_layer(p));
    let (mask_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_mask_gradient(p));
    let (scrollbar_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_scrollbar_color(p));
    let (translate_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_translate(p));
    let (spacing_props, own_props): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_border_spacing(p));

    let mut rules: Vec<String> = Vec::new();
    if !own_props.is_empty()
        || !shadow_props.is_empty()
        || !mask_props.is_empty()
        || !scrollbar_props.is_empty()
        || !translate_props.is_empty()
        || !spacing_props.is_empty()
    {
        let mut body = String::new();
        for prop in own_props {
            let (name, value) = property_and_value(prop);
            body.push_str(&format!("  {name}: {value};\n"));
        }
        if let Some(value) = box_shadow_value(&shadow_props) {
            body.push_str(&format!("  box-shadow: {value};\n"));
        }
        for (name, value) in mask_declarations(&mask_props) {
            body.push_str(&format!("  {name}: {value};\n"));
        }
        if let Some(value) = scrollbar_color_value(&scrollbar_props) {
            body.push_str(&format!("  scrollbar-color: {value};\n"));
        }
        if let Some(value) = translate_value(&translate_props) {
            body.push_str(&format!("  translate: {value};\n"));
        }
        if let Some(value) = border_spacing_value(&spacing_props) {
            body.push_str(&format!("  border-spacing: {value};\n"));
        }
        rules.push(format!(".{class_name}{suffix} {{\n{body}}}"));
    }
    if !child_props.is_empty() {
        let mut body = String::new();
        for prop in child_props {
            for (name, value) in space_declarations(prop) {
                body.push_str(&format!("  {name}: {value};\n"));
            }
        }
        // `:where()` keeps the specificity at zero, matching Tailwind, so
        // a child's own utilities still win over the parent's spacing.
        rules.push(format!(":where(.{class_name}{suffix} > :not(:last-child)) {{\n{body}}}"));
    }
    if !placeholder_props.is_empty() {
        let mut body = String::new();
        for prop in placeholder_props {
            let (name, value) = property_and_value(prop);
            body.push_str(&format!("  {name}: {value};\n"));
        }
        rules.push(format!(".{class_name}{suffix}::placeholder {{\n{body}}}"));
    }

    let rule = rules.join("\n\n");
    match media {
        Some(query) => format!("@media {query} {{\n{rule}\n}}"),
        None => rule,
    }
}

/// Escapes a class name for use in a CSS selector. Tailwind class names
/// contain characters that are selector syntax -- `hover:bg-blue-500`,
/// `w-1/2`, `p-1.5` -- and must be backslash-escaped to be matched
/// literally. Same escaping Tailwind's own output uses.
pub fn escape_class_selector(class_name: &str) -> String {
    let mut out = String::with_capacity(class_name.len());
    for c in class_name.chars() {
        if matches!(c, ':' | '/' | '.' | '[' | ']' | '%' | '!' | '#' | '(' | ')' | ',') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The declarations `space-x-*`/`space-y-*` put on each non-last child.
/// Both sides are written, not just the gap-bearing one, because Tailwind
/// does the same -- its reverse-direction support needs the zero side to
/// be explicit.
fn space_declarations(prop: &StyleProperty) -> Vec<(&'static str, String)> {
    match prop {
        StyleProperty::SpaceX(l) => vec![
            ("margin-inline-start", "0".to_string()),
            ("margin-inline-end", length_px(*l)),
        ],
        StyleProperty::SpaceY(l) => {
            vec![("margin-top", "0".to_string()), ("margin-bottom", length_px(*l))]
        }
        // Tailwind writes both edges, zeroing the leading one, so that
        // `divide-x-reverse` can flip which edge carries the border without
        // a different rule. Matching that shape keeps the output identical.
        StyleProperty::DivideX(l) => vec![
            ("border-inline-style", "solid".to_string()),
            ("border-inline-start-width", "0".to_string()),
            ("border-inline-end-width", length_px(*l)),
        ],
        StyleProperty::DivideY(l) => vec![
            ("border-bottom-style", "solid".to_string()),
            ("border-top-style", "solid".to_string()),
            ("border-top-width", "0".to_string()),
            ("border-bottom-width", length_px(*l)),
        ],
        StyleProperty::DivideColor(c) => vec![("border-color", color_var(c))],
        StyleProperty::DivideStyle(s) => {
            let keyword = border_style_keyword(s).to_string();
            vec![
                ("border-top-style", keyword.clone()),
                ("border-right-style", keyword.clone()),
                ("border-bottom-style", keyword.clone()),
                ("border-left-style", keyword),
            ]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dowel_ir::{ExprRef, SourceSpan};

    #[test]
    fn not_and_compose_into_a_selector() {
        let a = ConditionExpr::Ref(ExprRef(SourceSpan { start: 0, end: 1 }));
        let b = ConditionExpr::Ref(ExprRef(SourceSpan { start: 2, end: 3 }));
        let expr = ConditionExpr::And(Box::new(a), Box::new(ConditionExpr::Not(Box::new(b))));
        let (media, suffix) = condition_shape(&Condition::Expr(expr));
        assert!(media.is_none());
        assert_eq!(suffix, "[data-dowel-cond-0-1]:not([data-dowel-cond-2-3])");
    }

    #[test]
    fn known_color_token_resolves_to_real_oklch() {
        let (name, value) = property_and_value(&StyleProperty::BackgroundColor(Color::Token("blue-500".to_string())));
        assert_eq!(name, "background-color");
        assert_eq!(value, "oklch(62.3% 0.214 259.815)");
    }

    #[test]
    fn unknown_color_token_falls_back_to_a_css_custom_property() {
        let (_, value) =
            property_and_value(&StyleProperty::TextColor(Color::Token("brand-primary".to_string())));
        assert_eq!(value, "var(--dowel-color-brand-primary)");
    }
}
