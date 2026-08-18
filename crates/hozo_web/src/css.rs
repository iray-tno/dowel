//! `StyleProperty`/`Condition` -> CSS text.
//!
//! Grouping/flattening declarations by `Condition` lives in `hozo_ir`
//! (shared with `hozo_native`, which needs the identical rule -- "last
//! wins" only within one condition group). This module is just the
//! Web-specific value/selector formatting on top of that.
//!
//! `Color` stays a Tailwind token through the whole IR (proposal §16 still
//! defers *theme-aware* resolution to a later pass -- custom colors,
//! arbitrary values), but the default Tailwind palette is resolved here via
//! `hozo_ir::resolve_color_token`, emitted as the exact `oklch(...)`
//! string Tailwind's own CSS would produce. A token outside the default
//! palette still falls back to a CSS custom property reference
//! (`var(--hozo-color-x)`, never actually defined anywhere) --
//! correct-but-unresolved, not silently wrong.

use hozo_ir::{
    Align, AlignSelf, Angle, BorderStyle, Breakpoint, Clamp, Color, Condition, ConditionExpr,
    FilterFunction, Scale,
    DecorationStyle,
    ColumnCount, Dimension, Display, Edge, GradientStop, GridLine, GridSpan, GridTracks, MaskSlot,
    MaskStop,
    Em, FlexDirection, LetterSpacing, FlexShorthand, Justify, Length, LineHeight, Overflow, Position, Radius,
    StyleProperty, TextAlign, TextOverflow, TextTransform, Theme, WhiteSpace,
};

/// A length as CSS text, against the project's spacing scale.
///
/// Takes the theme because a spacing step's width is the project's to
/// decide -- see `Length::Spacing`.
fn length_px(length: Length, theme: &Theme) -> String {
    match length {
        // Printed in the unit it was written in, not converted. `w-[2rem]`
        // is `width: 2rem` in Tailwind's output, and the difference from
        // `32px` is not cosmetic -- a reader who has scaled up their
        // browser's font size gets a wider element from one and the same
        // element from the other. Converting here silently opted every
        // arbitrary `rem` out of that.
        Length::Unit(value, unit) => format!("{value}{}", unit.css()),
        _ => format!("{}px", length.px(theme)),
    }
}

fn dimension_value(dim: &Dimension, theme: &Theme) -> String {
    match dim {
        Dimension::Length(length) => length_px(*length, theme),
        Dimension::Percent(pct) => format!("{pct}%"),
        Dimension::Auto => "auto".to_string(),
        Dimension::ViewportWidth(pct) => format!("{pct}vw"),
        Dimension::ViewportHeight(pct) => format!("{pct}vh"),
        Dimension::Css(text) => text.to_string(),
    }
}

/// Tailwind writes equal tracks as `repeat(n, minmax(0, 1fr))`, not
/// `repeat(n, 1fr)`. The `minmax` floor is what stops an oversized item
/// from widening its own track, so the two behave differently under
/// overflow and this is not a formatting choice.
fn grid_tracks(tracks: &GridTracks) -> String {
    match tracks {
        GridTracks::Count(n) => format!("repeat({n}, minmax(0, 1fr))"),
        GridTracks::None => "none".to_string(),
        GridTracks::Subgrid => "subgrid".to_string(),
        GridTracks::Css(list) => list.clone(),
    }
}

fn grid_line(line: &GridLine) -> String {
    match line {
        GridLine::Line(n) => n.to_string(),
        GridLine::Auto => "auto".to_string(),
    }
}

fn grid_span(span: &GridSpan) -> String {
    match span {
        // Both edges, so the item spans n tracks wherever it lands rather
        // than being pinned to a line.
        GridSpan::Span(n) => format!("span {n} / span {n}"),
        GridSpan::Full => "1 / -1".to_string(),
        GridSpan::Auto => "auto".to_string(),
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
        Justify::Stretch => "stretch",
        Justify::Baseline => "baseline",
        Justify::Css(v) => v,
    }
}

/// The CSS longhand each logical edge writes. Spelled out rather than
/// built, because `property_and_value` returns `&'static str`.
fn border_width_property(edge: Edge) -> &'static str {
    match edge {
        Edge::Inline => "border-inline-width",
        Edge::Block => "border-block-width",
        Edge::InlineStart => "border-inline-start-width",
        Edge::InlineEnd => "border-inline-end-width",
        Edge::BlockStart => "border-block-start-width",
        Edge::BlockEnd => "border-block-end-width",
        // The physical edges have their own properties; see
        // `StyleProperty::BorderLogicalWidth`.
        _ => "border-width",
    }
}

fn border_style_property(edge: Edge) -> &'static str {
    match edge {
        Edge::Inline => "border-inline-style",
        Edge::Block => "border-block-style",
        Edge::InlineStart => "border-inline-start-style",
        Edge::InlineEnd => "border-inline-end-style",
        Edge::BlockStart => "border-block-start-style",
        Edge::BlockEnd => "border-block-end-style",
        _ => "border-style",
    }
}

/// The four declarations `line-clamp-*` is, as one group.
///
/// Tailwind writes all four because the `-webkit-line-clamp` mechanism
/// only works inside a `-webkit-box` with a vertical orientation and
/// hidden overflow -- they are one thing, not four choices.
fn line_clamp_declarations(lines: Option<&Clamp>) -> Vec<(&'static str, String)> {
    match lines {
        Some(n) => vec![
            ("overflow", "hidden".to_string()),
            ("display", "-webkit-box".to_string()),
            ("-webkit-box-orient", "vertical".to_string()),
            ("-webkit-line-clamp", n.css()),
        ],
        None => vec![
            ("overflow", "visible".to_string()),
            ("display", "block".to_string()),
            ("-webkit-box-orient", "horizontal".to_string()),
            ("-webkit-line-clamp", "unset".to_string()),
        ],
    }
}

fn overflow_keyword(overflow: &Overflow) -> &'static str {
    match overflow {
        Overflow::Visible => "visible",
        Overflow::Hidden => "hidden",
        Overflow::Scroll => "scroll",
        Overflow::Css(v) => v,
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

fn radius_value(radius: &Radius, theme: &Theme) -> String {
    match radius {
        Radius::Length(l) => length_px(*l, theme),
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
            | StyleProperty::MaskSlotArgument(..)
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
    angle: Option<Angle>,
}

/// A mask gradient's angle, defaulting to the `0deg` Tailwind's
/// `--tw-mask-*-position` register starts at.
fn mask_angle(angle: &Option<Angle>) -> String {
    angle.as_ref().map_or_else(|| "0deg".to_string(), Angle::css)
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
/// difference lives in which registers each one sets. Hozo has the whole
/// set in hand, so it writes the resolved list and ships no custom
/// properties.
///
/// The first slot is overloaded in Tailwind too: a side utility makes it a
/// four-layer list (left, right, bottom, top), a `mask-linear-*` makes it a
/// single gradient. Using both kinds together is therefore
/// order-dependent in Tailwind and resolved here as "sides win", which is
/// the only case where the two can disagree.
fn mask_declarations(props: &[&StyleProperty], theme: &Theme) -> Vec<(&'static str, String)> {
    let color_var = |color: &Color| resolve_theme_color(color, theme);
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
                    MaskStop::From => g.from_position = Some(dimension_value(d, theme)),
                    MaskStop::To => g.to_position = Some(dimension_value(d, theme)),
                },
                StyleProperty::MaskSlotArgument(s, degrees) if *s == slot => g.angle = Some(degrees.clone()),
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
            (true, false) => format!("linear-gradient({})", mask_angle(&linear.angle)),
            (true, true) => {
                format!("linear-gradient({}, {})", mask_angle(&linear.angle), linear.stops())
            }
        });
    }

    layers.push(if radial.is_set() && !radial.has_stops() {
        // An arbitrary size with nothing to shape: the size *is* the
        // gradient, the same way an angle alone is for the other two
        // slots.
        format!("radial-gradient({})", mask_angle(&radial.angle))
    } else if radial.is_set() {
        format!(
            "radial-gradient({} {} at {}, {})",
            keyword(
                |p| match p {
                    StyleProperty::MaskRadialShape(v) => Some(*v),
                    _ => None,
                },
                "ellipse"
            ),
            // Not through `keyword`, which returns `&'static str`: this
            // one carries an arbitrary size (`mask-radial-[10px]`) and so
            // has to be owned.
            props
                .iter()
                .find_map(|p| match p {
                    StyleProperty::MaskRadialSize(v) => Some(v.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| "farthest-corner".to_string()),
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
        (true, false) => format!("conic-gradient({})", mask_angle(&conic.angle)),
        (true, true) => {
            format!("conic-gradient(from {}, {})", mask_angle(&conic.angle), conic.stops())
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

/// The `filter`/`backdrop-filter` chain, in `FilterFunction` order.
///
/// Sorted by the function rather than by the order the utilities were
/// written, because filter functions don't commute -- `grayscale invert`
/// and `invert grayscale` render differently -- and Tailwind's own order is
/// fixed by the order its registers appear in the value. A slot cleared by
/// a `-none` utility contributes an empty register there and so nothing
/// here.
fn filter_value(props: &[&StyleProperty], backdrop: bool) -> Option<String> {
    let mut functions: Vec<(FilterFunction, &str)> = Vec::new();
    for prop in props {
        let (function, value) = match (prop, backdrop) {
            (StyleProperty::Filter(f, v), false) | (StyleProperty::BackdropFilter(f, v), true) => {
                (*f, v.as_str())
            }
            _ => continue,
        };
        // `filter-none` is the whole chain off, not one slot cleared.
        if function == FilterFunction::None {
            return Some("none".to_string());
        }
        functions.push((function, value));
    }
    if functions.is_empty() {
        return None;
    }
    functions.sort_by_key(|(function, _)| *function);
    let chain: Vec<&str> = functions.iter().map(|(_, v)| *v).filter(|v| !v.is_empty()).collect();
    // Every slot cleared still leaves a declaration -- Tailwind emits
    // `filter: ` with an empty value, which is what an all-`-none` chain
    // resolves to.
    Some(chain.join(" "))
}

fn is_gradient(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::Gradient(..)
            | StyleProperty::GradientStopColor(..)
            | StyleProperty::GradientStopPosition(..)
            | StyleProperty::BackgroundImageNone
    )
}

/// The `background-image` a gradient constructor and its stops make.
///
/// `None` without a constructor, which is the whole reason the stops are
/// separate properties: `from-red-500` alone paints nothing in Tailwind
/// either -- it fills a register that no `background-image` reads until a
/// `bg-linear-*` writes one.
///
/// Unwritten halves fall back to the `initial-value`s of Tailwind's own
/// registers: a missing colour is `#0000` and a missing position is
/// 0%/50%/100%. So `bg-linear-to-r from-red-500` really is a red-to-
/// transparent ramp rather than a half-specified gradient.
///
/// One difference from Tailwind, deliberate: it writes the prelude twice,
/// once plain and once inside `@supports (background-image:
/// linear-gradient(in lab, red, red))`, so an engine without colour-space
/// interpolation still gets a gradient. Hozo emits only the modern form,
/// the same call `Radius::Full` makes with `calc(infinity * 1px)`.
fn gradient_value(props: &[&StyleProperty], theme: &Theme) -> Option<String> {
    // `bg-none` and a gradient constructor write the same declaration, so
    // whichever came last wins -- which means scanning from the end. The
    // inner `Option` is the answer, the outer one is "did anything here
    // set a background image at all".
    let latest = props.iter().rev().find_map(|p| match p {
        StyleProperty::Gradient(kind, prelude) => Some(Some((*kind, prelude.clone()))),
        StyleProperty::BackgroundImageNone => Some(None),
        _ => None,
    })?;
    let Some((kind, prelude)) = latest else { return Some("none".to_string()) };

    let color = |stop: GradientStop| {
        props.iter().find_map(|p| match p {
            StyleProperty::GradientStopColor(s, c) if *s == stop => Some(resolve_theme_color(c, theme)),
            _ => None,
        })
    };
    let position = |stop: GradientStop| {
        props
            .iter()
            .find_map(|p| match p {
                StyleProperty::GradientStopPosition(s, d) if *s == stop => {
                    Some(dimension_value(d, theme))
                }
                _ => None,
            })
            .unwrap_or_else(|| stop.default_position().to_string())
    };
    let stop = |s: GradientStop| {
        format!("{} {}", color(s).unwrap_or_else(|| "#0000".to_string()), position(s))
    };

    // No stop *colour* anywhere means no stop list at all -- the prelude
    // is the whole value. That is Tailwind's register model rather than a
    // shortcut: only a `from-*`/`via-*`/`to-*` colour writes
    // `--tw-gradient-stops`, so a lone `bg-conic-[10px]` really is
    // `conic-gradient(10px)`, and a lone `bg-linear-to-r` really is a
    // gradient with nothing in it that the browser then drops.
    //
    // A stop *position* doesn't count, which is why the check is on
    // colours: `via-30%` sets a position for a stop that isn't in the
    // list, and changes nothing in Tailwind either.
    let stops: Vec<String> = if [GradientStop::From, GradientStop::Via, GradientStop::To]
        .iter()
        .all(|s| color(*s).is_none())
    {
        Vec::new()
    } else {
        let mut stops = vec![stop(GradientStop::From)];
        if color(GradientStop::Via).is_some() {
            stops.push(stop(GradientStop::Via));
        }
        stops.push(stop(GradientStop::To));
        stops
    };

    if stops.is_empty() {
        return Some(format!("{}({prelude})", kind.css()));
    }
    Some(format!("{}({prelude}, {})", kind.css(), stops.join(", ")))
}

fn is_filter(prop: &StyleProperty) -> bool {
    matches!(prop, StyleProperty::Filter(..) | StyleProperty::BackdropFilter(..))
}

fn is_scale_axis(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::ScaleX(_)
            | StyleProperty::ScaleY(_)
            | StyleProperty::ScaleZ(_)
            | StyleProperty::Scale3d
    )
}

fn is_transform_function(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::RotateX(_)
            | StyleProperty::RotateY(_)
            | StyleProperty::RotateZ(_)
            | StyleProperty::SkewX(_)
            | StyleProperty::SkewY(_)
    )
}

/// The per-axis scales as one `scale` declaration.
///
/// An unwritten axis is `1`, not `100%` -- that is the literal
/// `initial-value` of Tailwind's `--tw-scale-*` registers, and since a
/// written axis is a percentage the two spellings sit side by side in the
/// same declaration (`scale-x-50` is `50% 1`). They mean the same thing to
/// CSS; a differential test compares strings.
///
/// The third component appears only when a z-form utility was written --
/// see `StyleProperty::Scale3d` for why that isn't the same question as
/// whether the z axis has a value.
fn scale_value(props: &[&StyleProperty]) -> Option<String> {
    if props.is_empty() {
        return None;
    }
    let axis = |f: fn(&StyleProperty) -> Option<&Scale>| {
        props.iter().find_map(|p| f(p)).map_or_else(|| "1".to_string(), Scale::css)
    };
    let x = axis(|p| match p {
        StyleProperty::ScaleX(v) => Some(v),
        _ => None,
    });
    let y = axis(|p| match p {
        StyleProperty::ScaleY(v) => Some(v),
        _ => None,
    });
    if !props.iter().any(|p| matches!(p, StyleProperty::Scale3d)) {
        return Some(format!("{x} {y}"));
    }
    let z = axis(|p| match p {
        StyleProperty::ScaleZ(v) => Some(v),
        _ => None,
    });
    Some(format!("{x} {y} {z}"))
}

/// The 3D rotations and skews as one `transform` declaration.
///
/// Order matters and is Tailwind's: `rotateX rotateY rotateZ skewX skewY`,
/// which is the order its `--tw-*` registers appear in the value. Transform
/// functions don't commute, so a different order is a different rendering
/// rather than a different spelling.
fn transform_value(props: &[&StyleProperty]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut push = |name: &str, angle: Option<&Angle>| {
        if let Some(a) = angle {
            parts.push(format!("{name}({})", a.css()));
        }
    };
    let find = |f: fn(&StyleProperty) -> Option<&Angle>| props.iter().find_map(|p| f(p));
    push("rotateX", find(|p| match p {
        StyleProperty::RotateX(a) => Some(a),
        _ => None,
    }));
    push("rotateY", find(|p| match p {
        StyleProperty::RotateY(a) => Some(a),
        _ => None,
    }));
    push("rotateZ", find(|p| match p {
        StyleProperty::RotateZ(a) => Some(a),
        _ => None,
    }));
    push("skewX", find(|p| match p {
        StyleProperty::SkewX(a) => Some(a),
        _ => None,
    }));
    push("skewY", find(|p| match p {
        StyleProperty::SkewY(a) => Some(a),
        _ => None,
    }));
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// CSS's `translate` is one property taking up to three values, so the
/// three axes have to become one declaration. Emitting them separately --
/// which this did until 2026-08-15 -- made `translate-x-4 translate-y-8`
/// write two `translate:` declarations, and last-wins threw the x away.
///
/// The z value is only written when present, matching Tailwind: the
/// two-value form is what `translate-x-*`/`translate-y-*` produce.
fn translate_value(props: &[&StyleProperty], theme: &Theme) -> Option<String> {
    if props.is_empty() {
        return None;
    }
    // A trait object rather than a fn pointer: the closures below capture
    // `theme` now, so they aren't plain functions any more.
    let axis = |f: &dyn Fn(&StyleProperty) -> Option<String>| {
        props.iter().find_map(|p| f(p)).unwrap_or_else(|| "0".to_string())
    };
    let x = axis(&|p: &StyleProperty| match p {
        StyleProperty::TranslateX(d) => Some(dimension_value(d, theme)),
        _ => None,
    });
    let y = axis(&|p: &StyleProperty| match p {
        StyleProperty::TranslateY(d) => Some(dimension_value(d, theme)),
        _ => None,
    });
    let z = props.iter().find_map(|p| match p {
        StyleProperty::TranslateZ(d) => Some(dimension_value(d, theme)),
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
fn border_spacing_value(props: &[&StyleProperty], theme: &Theme) -> Option<String> {
    if props.is_empty() {
        return None;
    }
    let axis = |f: fn(&StyleProperty) -> Option<&Dimension>| {
        props
            .iter()
            .find_map(|p| f(p))
            .map(|d| dimension_value(d, theme))
            .unwrap_or_else(|| "0".to_string())
    };
    Some(format!(
        "{} {}",
        axis(|p| match p {
            StyleProperty::BorderSpacingX(d) => Some(d),
            _ => None,
        }),
        axis(|p| match p {
            StyleProperty::BorderSpacingY(d) => Some(d),
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
fn scrollbar_color_value(props: &[&StyleProperty], theme: &Theme) -> Option<String> {
    let find = |f: fn(&StyleProperty) -> Option<&Color>| {
        props
            .iter()
            .find_map(|p| f(p))
            .map(|color| resolve_theme_color(color, theme))
            .unwrap_or_else(|| "#0000".to_string())
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

fn resolve_theme_color(color: &Color, theme: &Theme) -> String {
    let token = match color {
        Color::Token(token) => token,
        // Not a colour at all, so it does not go through the palette.
        Color::Keyword(keyword) => return keyword.to_string(),
        // Already a colour. An arbitrary value is the author saying "not
        // the palette", so looking it up would be answering a question
        // nobody asked -- and answering it wrong, since `#ff0000` names
        // nothing in any theme and would come back as
        // `var(--hozo-color-#ff0000)`.
        Color::Css(text) => return text.clone(),
    };
    match theme.color(token) {
        Some(resolved) => resolved.oklch,
        // Not in the project's theme either. Still a reference rather than
        // a guess: correct-but-unresolved, which is what this has always
        // done for a token nothing defines.
        None => format!("var(--hozo-color-{token})"),
    }
}

/// Maps one `StyleProperty` to a `(css-property-name, value)` pair. Values
/// mirror Tailwind's own generated CSS where there's a choice (e.g.
/// `align-items: flex-start` rather than the newer `start` keyword) so
/// output stays recognizable to anyone used to reading Tailwind's CSS.
pub fn property_and_value<'a>(prop: &'a StyleProperty, theme: &Theme) -> (&'a str, String) {
    // Bound once, so the thirty-odd colour arms below read exactly as they
    // did before a theme existed. Threading `theme` through each of them
    // would add a word to thirty lines and say nothing at any of them.
    let color_var = |color: &Color| resolve_theme_color(color, theme);
    match prop {
        // Straight through, both halves. Neither is checked against
        // anything -- see `StyleProperty::Arbitrary` for why that is the
        // deal an arbitrary property makes rather than an omission.
        StyleProperty::Arbitrary(property, value) => (property.as_str(), value.clone()),
        StyleProperty::Display(d) => (
            "display",
            match d {
                Display::Flex => "flex",
                Display::None => "none",
                Display::Contents => "contents",
                Display::Block => "block",
                Display::InlineFlex => "inline-flex",
                Display::Grid => "grid",
                Display::Css(keyword) => keyword,
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
                FlexShorthand::Fraction(n, d) => format!("calc({n}/{d} * 100%)"),
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
                Align::Css(v) => v,
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
                AlignSelf::Css(v) => v,
            }
            .to_string(),
        ),
        StyleProperty::AlignContent(justify) => ("align-content", justify_keyword(justify).to_string()),
        StyleProperty::JustifyContent(justify) => {
            ("justify-content", justify_keyword(justify).to_string())
        }
        StyleProperty::Gap(l) => ("gap", length_px(*l, theme)),
        StyleProperty::RowGap(l) => ("row-gap", length_px(*l, theme)),
        StyleProperty::ColumnGap(l) => ("column-gap", length_px(*l, theme)),
        StyleProperty::MarginTop(d) => ("margin-top", dimension_value(d, theme)),
        StyleProperty::MarginRight(d) => ("margin-right", dimension_value(d, theme)),
        StyleProperty::MarginBottom(d) => ("margin-bottom", dimension_value(d, theme)),
        StyleProperty::MarginLeft(d) => ("margin-left", dimension_value(d, theme)),
        StyleProperty::PaddingTop(l) => ("padding-top", length_px(*l, theme)),
        StyleProperty::PaddingRight(l) => ("padding-right", length_px(*l, theme)),
        StyleProperty::PaddingBottom(l) => ("padding-bottom", length_px(*l, theme)),
        StyleProperty::PaddingLeft(l) => ("padding-left", length_px(*l, theme)),
        StyleProperty::MarginInlineStart(d) => ("margin-inline-start", dimension_value(d, theme)),
        StyleProperty::MarginInlineEnd(d) => ("margin-inline-end", dimension_value(d, theme)),
        StyleProperty::PaddingInlineStart(l) => ("padding-inline-start", length_px(*l, theme)),
        StyleProperty::PaddingInlineEnd(l) => ("padding-inline-end", length_px(*l, theme)),
        StyleProperty::Width(d) => ("width", dimension_value(d, theme)),
        StyleProperty::Height(d) => ("height", dimension_value(d, theme)),
        StyleProperty::MinWidth(d) => ("min-width", dimension_value(d, theme)),
        StyleProperty::MinHeight(d) => ("min-height", dimension_value(d, theme)),
        StyleProperty::MaxWidth(d) => ("max-width", dimension_value(d, theme)),
        StyleProperty::MaxHeight(d) => ("max-height", dimension_value(d, theme)),
        StyleProperty::ZIndex(z) => (
            "z-index",
            z.map_or_else(|| "auto".to_string(), |z| z.to_string()),
        ),
        StyleProperty::GridTemplateColumns(t) => ("grid-template-columns", grid_tracks(t)),
        StyleProperty::GridTemplateRows(t) => ("grid-template-rows", grid_tracks(t)),
        StyleProperty::GridColumnStart(l) => ("grid-column-start", grid_line(l)),
        StyleProperty::GridColumnEnd(l) => ("grid-column-end", grid_line(l)),
        StyleProperty::GridRowStart(l) => ("grid-row-start", grid_line(l)),
        StyleProperty::GridRowEnd(l) => ("grid-row-end", grid_line(l)),
        StyleProperty::GridColumn(s) => ("grid-column", grid_span(s)),
        StyleProperty::GridRow(s) => ("grid-row", grid_span(s)),
        StyleProperty::Position(pos) => (
            "position",
            match pos {
                Position::Relative => "relative",
                Position::Absolute => "absolute",
                Position::Static => "static",
                Position::Css(v) => v,
            }
            .to_string(),
        ),
        StyleProperty::InsetTop(d) => ("top", dimension_value(d, theme)),
        StyleProperty::InsetRight(d) => ("right", dimension_value(d, theme)),
        StyleProperty::InsetBottom(d) => ("bottom", dimension_value(d, theme)),
        StyleProperty::InsetLeft(d) => ("left", dimension_value(d, theme)),
        StyleProperty::InsetInlineStart(d) => ("inset-inline-start", dimension_value(d, theme)),
        StyleProperty::InsetInlineEnd(d) => ("inset-inline-end", dimension_value(d, theme)),
        StyleProperty::InsetInline(d) => ("inset-inline", dimension_value(d, theme)),
        StyleProperty::InsetBlock(d) => ("inset-block", dimension_value(d, theme)),
        StyleProperty::InsetBlockStart(d) => ("inset-block-start", dimension_value(d, theme)),
        StyleProperty::InsetBlockEnd(d) => ("inset-block-end", dimension_value(d, theme)),
        StyleProperty::BackgroundColor(c) => ("background-color", color_var(c)),
        StyleProperty::Opacity(o) => ("opacity", format!("{o}")),
        StyleProperty::BorderColor(c) => ("border-color", color_var(c)),
        StyleProperty::ScrollMargin(edge, l) => (scroll_margin_property(*edge), length_px(*l, theme)),
        StyleProperty::ScrollPadding(edge, l) => (scroll_padding_property(*edge), length_px(*l, theme)),
        StyleProperty::ScrollBehavior(value) => ("scroll-behavior", value.to_string()),
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
        // Composed by `gradient_value`, partitioned out before this runs.
        StyleProperty::BackgroundImageNone
        | StyleProperty::Gradient(..)
        | StyleProperty::GradientStopColor(..)
        | StyleProperty::GradientStopPosition(..) => ("background-image", String::new()),
        // Composed by `mask_declarations`; `render_rule` partitions these
        // out before this runs.
        StyleProperty::MaskStopColor(..)
        | StyleProperty::MaskStopPosition(..)
        | StyleProperty::MaskSlotArgument(..)
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
        StyleProperty::TextDecorationThickness(l) => ("text-decoration-thickness", length_px(*l, theme)),
        // Emitted into its own `::placeholder` rule by `render_rule`.
        StyleProperty::PlaceholderColor(c) => ("color", color_var(c)),
        StyleProperty::OutlineWidth(l) => ("outline-width", length_px(*l, theme)),
        StyleProperty::OutlineStyle(s) => ("outline-style", border_style_keyword(s).to_string()),
        StyleProperty::OutlineColor(c) => ("outline-color", color_var(c)),
        StyleProperty::OutlineOffset(l) => ("outline-offset", length_px(*l, theme)),
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
        StyleProperty::BorderTopWidth(l) => ("border-top-width", length_px(*l, theme)),
        StyleProperty::BorderRightWidth(l) => ("border-right-width", length_px(*l, theme)),
        StyleProperty::BorderBottomWidth(l) => ("border-bottom-width", length_px(*l, theme)),
        StyleProperty::BorderLeftWidth(l) => ("border-left-width", length_px(*l, theme)),
        StyleProperty::BorderTopStyle(s) => ("border-top-style", border_style_keyword(s).to_string()),
        StyleProperty::BorderRightStyle(s) => {
            ("border-right-style", border_style_keyword(s).to_string())
        }
        StyleProperty::BorderBottomStyle(s) => {
            ("border-bottom-style", border_style_keyword(s).to_string())
        }
        StyleProperty::BorderLeftStyle(s) => ("border-left-style", border_style_keyword(s).to_string()),
        StyleProperty::BorderRadius(r) => ("border-radius", radius_value(r, theme)),
        StyleProperty::BorderTopLeftRadius(r) => ("border-top-left-radius", radius_value(r, theme)),
        StyleProperty::BorderTopRightRadius(r) => ("border-top-right-radius", radius_value(r, theme)),
        StyleProperty::BorderBottomRightRadius(r) => ("border-bottom-right-radius", radius_value(r, theme)),
        StyleProperty::BorderBottomLeftRadius(r) => ("border-bottom-left-radius", radius_value(r, theme)),
        StyleProperty::BorderStartStartRadius(r) => ("border-start-start-radius", radius_value(r, theme)),
        StyleProperty::BorderStartEndRadius(r) => ("border-start-end-radius", radius_value(r, theme)),
        StyleProperty::BorderEndStartRadius(r) => ("border-end-start-radius", radius_value(r, theme)),
        StyleProperty::BorderEndEndRadius(r) => ("border-end-end-radius", radius_value(r, theme)),
        StyleProperty::FontSize(l) => ("font-size", length_px(*l, theme)),
        StyleProperty::FontWeight(w) => ("font-weight", format!("{}", w.0)),
        StyleProperty::LineHeight(lh) => (
            "line-height",
            match lh {
                LineHeight::Length(l) => length_px(*l, theme),
                LineHeight::Ratio(r) => format!("{r}"),
            },
        ),
        StyleProperty::TextUnderlineOffset(d) => ("text-underline-offset", dimension_value(d, theme)),
        StyleProperty::OverflowX(o) => ("overflow-x", overflow_keyword(o).to_string()),
        StyleProperty::OverflowY(o) => ("overflow-y", overflow_keyword(o).to_string()),
        StyleProperty::BorderLogicalWidth(edge, l) => (border_width_property(*edge), length_px(*l, theme)),
        StyleProperty::BorderLogicalStyle(edge, s) => {
            (border_style_property(*edge), border_style_keyword(s).to_string())
        }
        // Composed; see `line_clamp_declarations`.
        StyleProperty::RotateNone => ("rotate", "none".to_string()),
        StyleProperty::ScaleNone => ("scale", "none".to_string()),
        StyleProperty::TranslateNone => ("translate", "none".to_string()),
        StyleProperty::TransformNone => ("transform", "none".to_string()),
        StyleProperty::TransformEmpty => ("transform", String::new()),
        StyleProperty::TransformGpu => ("transform", "translateZ(0)".to_string()),
        StyleProperty::LineClamp(_) => ("-webkit-line-clamp", String::new()),
        StyleProperty::FlexGrow(n) => ("flex-grow", format!("{n}")),
        StyleProperty::FlexShrink(n) => ("flex-shrink", format!("{n}")),
        StyleProperty::AspectRatio(v) => ("aspect-ratio", v.to_string()),
        StyleProperty::ObjectFit(v) => ("object-fit", v.to_string()),
        // Composed above: it writes the -webkit- prefix too.
        StyleProperty::UserSelect(_) => ("user-select", String::new()),
        StyleProperty::TextDecorationLine(v) => ("text-decoration-line", v.to_string()),
        StyleProperty::Keyword(property, value) => (property, value.to_string()),
        // Composed: it writes both halves. See `KeywordPair`.
        StyleProperty::KeywordPair(..) => ("", String::new()),
        StyleProperty::MixBlendMode(m) => ("mix-blend-mode", m.to_string()),
        StyleProperty::BackgroundBlendMode(m) => ("background-blend-mode", m.to_string()),
        StyleProperty::Order(n) => ("order", n.to_string()),
        StyleProperty::Cursor(keyword) => ("cursor", keyword.clone()),
        StyleProperty::Columns(columns) => (
            "columns",
            match columns {
                ColumnCount::Count(n) => n.to_string(),
                ColumnCount::Width(d) => dimension_value(d, theme),
                ColumnCount::Auto => "auto".to_string(),
            },
        ),
        StyleProperty::LetterSpacing(ls) => ("letter-spacing", match ls {
            LetterSpacing::Em(Em(v)) => format!("{v}em"),
            LetterSpacing::Px(l) => length_px(*l, theme),
        }),
        StyleProperty::Overflow(o) => ("overflow", overflow_keyword(o).to_string()),
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
                WhiteSpace::Css(v) => v,
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
                TextAlign::Css(v) => v,
            }
            .to_string(),
        ),
        // Standalone properties, as CSS defines them and Tailwind emits
        // them -- the `transform` shorthand isn't used on either side.
        // Tailwind writes both axes explicitly for scale/translate, so
        // these do the same rather than relying on one-value expansion.
        StyleProperty::Rotate(a) => ("rotate", a.css()),
        // Composed, not emitted here -- see `scale_value` / `transform_value`.
        StyleProperty::ScaleX(_)
        | StyleProperty::ScaleY(_)
        | StyleProperty::ScaleZ(_)
        | StyleProperty::Scale3d => ("scale", String::new()),
        StyleProperty::RotateX(_)
        | StyleProperty::RotateY(_)
        | StyleProperty::RotateZ(_)
        | StyleProperty::SkewX(_)
        | StyleProperty::SkewY(_) => ("transform", String::new()),
        // Composed by `translate_value`; partitioned out above.
        StyleProperty::TranslateX(_) | StyleProperty::TranslateY(_) | StyleProperty::TranslateZ(_) => {
            ("translate", String::new())
        }
        StyleProperty::FlexBasis(d) => ("flex-basis", dimension_value(d, theme)),
        StyleProperty::BlockSize(d) => ("block-size", dimension_value(d, theme)),
        StyleProperty::InlineSize(d) => ("inline-size", dimension_value(d, theme)),
        StyleProperty::MaxBlockSize(d) => ("max-block-size", dimension_value(d, theme)),
        StyleProperty::MaxInlineSize(d) => ("max-inline-size", dimension_value(d, theme)),
        StyleProperty::MinBlockSize(d) => ("min-block-size", dimension_value(d, theme)),
        StyleProperty::MinInlineSize(d) => ("min-inline-size", dimension_value(d, theme)),
        StyleProperty::TextIndent(d) => ("text-indent", dimension_value(d, theme)),
        StyleProperty::MarginBlockStart(d) => ("margin-block-start", dimension_value(d, theme)),
        StyleProperty::MarginBlockEnd(d) => ("margin-block-end", dimension_value(d, theme)),
        StyleProperty::PaddingBlockStart(l) => ("padding-block-start", length_px(*l, theme)),
        StyleProperty::PaddingBlockEnd(l) => ("padding-block-end", length_px(*l, theme)),
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
        | StyleProperty::RingOffsetWidth(_)
        | StyleProperty::RingOffsetColor(_)
        | StyleProperty::ShadowColor(_)
        | StyleProperty::InsetShadowColor(_)
        | StyleProperty::InsetRingColor(_)
        | StyleProperty::InsetShadow(_) => ("box-shadow", String::new()),
        // Composed, not emitted here -- see `filter_value`.
        StyleProperty::Filter(..) => ("filter", String::new()),
        StyleProperty::BackdropFilter(..) => ("backdrop-filter", String::new()),
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
pub fn expr_ref_attribute(expr_ref: hozo_ir::ExprRef) -> String {
    format!("data-hozo-cond-{}-{}", expr_ref.0.start, expr_ref.0.end)
}

fn condition_expr_selector(expr: &ConditionExpr) -> String {
    match expr {
        ConditionExpr::Static(true) => String::new(),
        ConditionExpr::Static(false) => ":not(*)".to_string(),
        ConditionExpr::Ref(expr_ref) => format!("[{}]", expr_ref_attribute(*expr_ref)),
        ConditionExpr::Not(inner) => format!(":not({})", condition_expr_selector(inner)),
        ConditionExpr::And(a, b) => format!("{}{}", condition_expr_selector(a), condition_expr_selector(b)),
        ConditionExpr::Or(a, b) => {
            format!(":is({}, {})", condition_expr_selector(a), condition_expr_selector(b))
        }
    }
}

/// A condition's shape as `(at-rule prelude, selector template)`.
///
/// The template carries `&` where the element's own class goes, the same
/// convention Tailwind and nested CSS use. It was a plain suffix until
/// arbitrary variants arrived, which a suffix cannot express: `[&>*]:p-4`
/// happens to append (`.hozo-0>*`) but `[.dark_&]:text-white` does not --
/// the element's class lands at the *end* there, and a suffix has no way
/// to say so.
///
/// The first half is a whole at-rule rather than a bare media query for
/// the same reason. `[@supports(display:grid)]:grid` is not a media query,
/// and hardcoding `@media` at the emission site would have made supports
/// queries unreachable by construction.
pub fn condition_shape(condition: &Condition) -> (Vec<String>, String) {
    match condition {
        // Stacked variants. The at-rules nest in written order, outermost
        // first, and the selector suffixes append in the same order --
        // `first:hover:` is `:first-child:hover`, not the reverse.
        //
        // Which is why the fold substitutes the accumulated selector *into*
        // each new template rather than the other way round: a template's
        // `&` is where the thing it qualifies goes, so the later variant
        // wraps the earlier one.
        Condition::All(conditions) => {
            let mut preludes = Vec::new();
            let mut selector = "&".to_string();
            for condition in conditions {
                let (mut inner, template) = condition_shape(condition);
                preludes.append(&mut inner);
                selector = template.replace('&', &selector);
            }
            (preludes, selector)
        }
        Condition::Always => (Vec::new(), "&".to_string()),
        // The capability query is not decoration. Without it a `:hover`
        // style sticks on a touch device after a tap -- the element keeps
        // matching until something else is tapped -- which is why Tailwind
        // v4 wraps every hover utility this way. Hozo emitted the bare
        // pseudo-class until 2026-08-17, and nothing noticed because no
        // comparison here looked at at-rules.
        Condition::Hover => (vec!["@media (hover: hover)".to_string()], "&:hover".to_string()),
        Condition::Focus => (Vec::new(), "&:focus".to_string()),
        Condition::FocusVisible => (Vec::new(), "&:focus-visible".to_string()),
        Condition::LastChild => (Vec::new(), "&:last-child".to_string()),
        // Only meaningful on elements that can actually be disabled (e.g.
        // <button>) -- CSS itself won't apply `:disabled` to a plain <div>.
        Condition::Disabled => (Vec::new(), "&:disabled".to_string()),
        // Known gotcha, not fixed here: iOS Safari doesn't reliably fire
        // `:active` from a tap unless the element has some touch-event
        // listener attached (a long-documented WebKit quirk). Hozo's
        // compiled onClick doesn't count. Fine for the common desktop/
        // Android case; tracked as a real gap, not silently "handled."
        Condition::Pressed => (Vec::new(), "&:active".to_string()),
        Condition::Responsive(bp) => (
            vec![format!("@media (min-width: {}px)", breakpoint_min_width_px(*bp))],
            "&".to_string(),
        ),
        // Tailwind v4's default dark strategy, and the one whose meaning
        // React Native's `useColorScheme()` shares.
        Condition::Dark => {
            (vec!["@media (prefers-color-scheme: dark)".to_string()], "&".to_string())
        }
        Condition::FirstChild => (Vec::new(), "&:first-child".to_string()),
        // Passed through exactly as written. Hozo does not parse it and
        // deliberately so: a selector it doesn't recognise is one the
        // browser may well support, and the author reached past the design
        // system on purpose. Validating it here would mean maintaining a
        // second, worse copy of the CSS selector grammar.
        Condition::ArbitrarySelector(selector) => (Vec::new(), selector.clone()),
        Condition::ArbitraryAtRule(rule) => (vec![rule.clone()], "&".to_string()),
        Condition::Expr(expr) => (Vec::new(), format!("&{}", condition_expr_selector(expr))),
    }
}

/// Fills a selector template in for one element.
///
/// `&` is replaced everywhere it appears, not just once: `[&+&]` is a
/// legitimate selector for "this element following another of itself".
fn fill_selector(template: &str, class_name: &str) -> String {
    template.replace('&', &format!(".{class_name}"))
}

/// Renders one CSS rule (optionally media-wrapped) for a class + condition
/// group's already-deduped properties.
fn is_shadow_layer(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::BoxShadow(_)
            | StyleProperty::InsetShadow(_)
            | StyleProperty::RingWidth(_)
            | StyleProperty::RingColor(_)
            | StyleProperty::InsetRingWidth(_)
            | StyleProperty::RingOffsetWidth(_)
            | StyleProperty::RingOffsetColor(_)
            | StyleProperty::ShadowColor(_)
            | StyleProperty::InsetShadowColor(_)
            | StyleProperty::InsetRingColor(_)
    )
}

/// Joins whichever ring/shadow utilities are present into one `box-shadow`.
///
/// Tailwind does this at runtime with `--tw-*` registers spliced into a
/// fixed layer list; Hozo knows the whole set at compile time, so it
/// writes the resolved list directly and ships no custom properties. Layer
/// order follows Tailwind's: inset ring, then ring, then the shadow.
///
/// A ring colour with no width contributes nothing, which is correct --
/// `ring-blue-500` alone has nothing to paint, exactly as in Tailwind.
/// Repaints every layer of a shadow in `color`, or leaves it alone if no
/// `shadow-<colour>` was written.
///
/// The shadow's layers carry a default colour each (`rgb(0 0 0 / 0.1)` and
/// friends), and a colour utility replaces all of them at once -- which is
/// what makes `shadow-lg shadow-blue-500` one blue shadow rather than a
/// black one with a colour beside it. Tailwind does the same substitution
/// through `--tw-shadow-color`.
///
/// The colour is the tail of each layer, so it is cut at the last `rgb(`
/// rather than parsed. That holds because the table this reads from is in
/// this repository: every shadow Hozo knows is written with an `rgb()`
/// colour last. An arbitrary `shadow-[…]` has no default colour to
/// replace and is left as written.
fn repaint_shadow(shadow: &str, color: Option<&Color>, theme: &Theme) -> String {
    let Some(color) = color else { return shadow.to_string() };
    let paint = resolve_theme_color(color, theme);
    split_layers(shadow)
        .into_iter()
        .map(|layer| match layer.rfind("rgb(") {
            Some(cut) => format!("{}{paint}", &layer[..cut]),
            None => layer.to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A comma-separated shadow list, split on the commas *between* layers --
/// `rgb(0 0 0 / 0.1)` has none, but `color-mix(in oklab, …)` does.
fn split_layers(value: &str) -> Vec<&str> {
    let mut layers = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, ch) in value.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                layers.push(value[start..index].trim());
                start = index + 1;
            }
            _ => {}
        }
    }
    layers.push(value[start..].trim());
    layers
}

fn box_shadow_value(props: &[&StyleProperty], theme: &Theme) -> Option<String> {
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
    let paint = |c: Option<&Color>| {
        c.map_or_else(|| "currentcolor".to_string(), |color| resolve_theme_color(color, theme))
    };

    let inset_shadow = props.iter().find_map(|p| match p {
        StyleProperty::InsetShadow(s) => Some(s.clone()),
        _ => None,
    });

    let ring_offset = find_length(|p| match p {
        StyleProperty::RingOffsetWidth(l) => Some(*l),
        _ => None,
    });
    let ring_offset_color = find_color(|p| match p {
        StyleProperty::RingOffsetColor(c) => Some(c),
        _ => None,
    });
    let shadow_color = find_color(|p| match p {
        StyleProperty::ShadowColor(c) => Some(c),
        _ => None,
    });
    let inset_shadow_color = find_color(|p| match p {
        StyleProperty::InsetShadowColor(c) => Some(c),
        _ => None,
    });

    let mut layers: Vec<String> = Vec::new();
    // Innermost first, matching the order Tailwind splices its registers:
    // inset shadow, inset ring, ring offset, ring, outer shadow.
    if let Some(shadow) = inset_shadow {
        layers.push(repaint_shadow(&shadow, inset_shadow_color, theme));
    }
    if let Some(width) = inset_ring {
        layers.push(format!("inset 0 0 0 {} {}", length_px(width, theme), paint(inset_ring_color)));
    }
    // The offset is drawn *under* the ring, in the page's own colour, and
    // pushes the ring outwards by its own width. Tailwind's register
    // default is white, which is the assumption that the element sits on
    // a white page -- inherited here rather than second-guessed.
    if let Some(width) = ring_offset {
        layers.push(format!(
            "0 0 0 {} {}",
            length_px(width, theme),
            ring_offset_color
                .map_or_else(|| "#fff".to_string(), |color| resolve_theme_color(color, theme)),
        ));
    }
    if let Some(width) = ring {
        // The ring's spread includes the offset: the two layers are
        // concentric, so the outer one has to clear the inner.
        let spread = match ring_offset {
            Some(offset) => length_px(Length::Px(width.px(theme) + offset.px(theme)), theme),
            None => length_px(width, theme),
        };
        layers.push(format!("0 0 0 {spread} {}", paint(ring_color)));
    }
    if let Some(shadow) = shadow {
        // `shadow-none` removes the *shadow* layer, not the whole
        // declaration -- `shadow-none ring-2` still draws the ring, which is
        // what Tailwind does by clearing only its `--tw-shadow` register.
        if shadow != "none" {
            layers.push(repaint_shadow(&shadow, shadow_color, theme));
        } else if layers.is_empty() {
            return Some("none".to_string());
        }
    }
    (!layers.is_empty()).then(|| layers.join(", "))
}

pub fn render_rule(
    class_name: &str,
    condition: &Condition,
    props: &[StyleProperty],
    theme: &Theme,
) -> String {
    let (at_rule, selector) = condition_shape(condition);
    let target = fill_selector(&selector, class_name);

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
    let (gradient_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_gradient(p));
    let (scrollbar_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_scrollbar_color(p));
    let (translate_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_translate(p));
    let (keyword_pair_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| matches!(p, StyleProperty::KeywordPair(..)));
    let (line_clamp_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| matches!(p, StyleProperty::LineClamp(_)));
    let (user_select_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| matches!(p, StyleProperty::UserSelect(_)));
    let (filter_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_filter(p));
    let (scale_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_scale_axis(p));
    let (transform_props, rest): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_transform_function(p));
    let (spacing_props, own_props): (Vec<&StyleProperty>, Vec<&StyleProperty>) =
        rest.into_iter().partition(|p| is_border_spacing(p));

    let mut rules: Vec<String> = Vec::new();
    if !own_props.is_empty()
        || !shadow_props.is_empty()
        || !mask_props.is_empty()
        || !gradient_props.is_empty()
        || !scrollbar_props.is_empty()
        || !translate_props.is_empty()
        || !keyword_pair_props.is_empty()
        || !line_clamp_props.is_empty()
        || !user_select_props.is_empty()
        || !filter_props.is_empty()
        || !scale_props.is_empty()
        || !transform_props.is_empty()
        || !spacing_props.is_empty()
    {
        let mut body = String::new();
        for prop in own_props {
            let (name, value) = property_and_value(prop, theme);
            body.push_str(&format!("  {name}: {value};\n"));
        }
        if let Some(value) = box_shadow_value(&shadow_props, theme) {
            body.push_str(&format!("  box-shadow: {value};\n"));
        }
        if let Some(value) = gradient_value(&gradient_props, theme) {
            body.push_str(&format!("  background-image: {value};
"));
        }
        for (name, value) in mask_declarations(&mask_props, theme) {
            body.push_str(&format!("  {name}: {value};\n"));
        }
        if let Some(value) = scrollbar_color_value(&scrollbar_props, theme) {
            body.push_str(&format!("  scrollbar-color: {value};\n"));
        }
        if let Some(value) = translate_value(&translate_props, theme) {
            body.push_str(&format!("  translate: {value};\n"));
        }
        if let Some(value) = filter_value(&filter_props, false) {
            body.push_str(&format!("  filter: {value};
"));
        }
        if let Some(value) = filter_value(&filter_props, true) {
            // The unprefixed property is not enough: Safari still ships
            // backdrop-filter only behind the -webkit- prefix, and Tailwind
            // emits both.
            body.push_str(&format!("  -webkit-backdrop-filter: {value};
"));
            body.push_str(&format!("  backdrop-filter: {value};
"));
        }
        for prop in &keyword_pair_props {
            if let StyleProperty::KeywordPair(p1, v1, p2, v2) = prop {
                body.push_str(&format!("  {p1}: {v1};
"));
                body.push_str(&format!("  {p2}: {v2};
"));
            }
        }
        for prop in &line_clamp_props {
            if let StyleProperty::LineClamp(lines) = prop {
                for (name, value) in line_clamp_declarations(lines.as_ref()) {
                    body.push_str(&format!("  {name}: {value};
"));
                }
            }
        }
        // Safari still needs the prefix for user-select, and Tailwind emits
        // both, so this is one utility writing two declarations rather than
        // a value formatting choice.
        for prop in &user_select_props {
            if let StyleProperty::UserSelect(value) = prop {
                body.push_str(&format!("  -webkit-user-select: {value};
"));
                body.push_str(&format!("  user-select: {value};
"));
            }
        }
        if let Some(value) = scale_value(&scale_props) {
            body.push_str(&format!("  scale: {value};\n"));
        }
        if let Some(value) = transform_value(&transform_props) {
            body.push_str(&format!("  transform: {value};\n"));
        }
        if let Some(value) = border_spacing_value(&spacing_props, theme) {
            body.push_str(&format!("  border-spacing: {value};\n"));
        }
        rules.push(format!("{target} {{\n{body}}}"));
    }
    if !child_props.is_empty() {
        let mut body = String::new();
        for prop in child_props {
            for (name, value) in space_declarations(prop, theme) {
                body.push_str(&format!("  {name}: {value};\n"));
            }
        }
        // `:where()` keeps the specificity at zero, matching Tailwind, so
        // a child's own utilities still win over the parent's spacing.
        rules.push(format!(":where({target} > :not(:last-child)) {{\n{body}}}"));
    }
    if !placeholder_props.is_empty() {
        let mut body = String::new();
        for prop in placeholder_props {
            let (name, value) = property_and_value(prop, theme);
            body.push_str(&format!("  {name}: {value};\n"));
        }
        rules.push(format!("{target}::placeholder {{\n{body}}}"));
    }

    // Nested innermost-last, so the first variant written is the outermost
    // wrapper -- `md:hover:` is a width query around a hover query, which
    // is how Tailwind writes it too.
    at_rule.into_iter().rev().fold(rules.join("\n\n"), |rule, prelude| {
        format!("{prelude} {{\n{rule}\n}}")
    })
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
fn space_declarations(prop: &StyleProperty, theme: &Theme) -> Vec<(&'static str, String)> {
    let color_var = |color: &Color| resolve_theme_color(color, theme);
    match prop {
        StyleProperty::SpaceX(l) => vec![
            ("margin-inline-start", "0".to_string()),
            ("margin-inline-end", dimension_value(l, theme)),
        ],
        StyleProperty::SpaceY(l) => {
            vec![("margin-top", "0".to_string()), ("margin-bottom", dimension_value(l, theme))]
        }
        // Tailwind writes both edges, zeroing the leading one, so that
        // `divide-x-reverse` can flip which edge carries the border without
        // a different rule. Matching that shape keeps the output identical.
        StyleProperty::DivideX(l) => vec![
            ("border-inline-style", "solid".to_string()),
            ("border-inline-start-width", "0".to_string()),
            ("border-inline-end-width", dimension_value(l, theme)),
        ],
        StyleProperty::DivideY(l) => vec![
            ("border-bottom-style", "solid".to_string()),
            ("border-top-style", "solid".to_string()),
            ("border-top-width", "0".to_string()),
            ("border-bottom-width", dimension_value(l, theme)),
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
    use hozo_ir::{ExprRef, SourceSpan};

    #[test]
    fn not_and_compose_into_a_selector() {
        let a = ConditionExpr::Ref(ExprRef(SourceSpan { start: 0, end: 1 }));
        let b = ConditionExpr::Ref(ExprRef(SourceSpan { start: 2, end: 3 }));
        let expr = ConditionExpr::And(Box::new(a), Box::new(ConditionExpr::Not(Box::new(b))));
        let (at_rule, selector) = condition_shape(&Condition::Expr(expr));
        assert!(at_rule.is_empty());
        assert_eq!(selector, "&[data-hozo-cond-0-1]:not([data-hozo-cond-2-3])");
    }

    #[test]
    fn an_arbitrary_selector_places_the_element_where_the_ampersand_is() {
        // The reason `condition_shape` returns a template rather than a
        // suffix: here the element's class lands at the end, and there is
        // no suffix that can say that.
        let (at_rule, selector) =
            condition_shape(&Condition::ArbitrarySelector(".dark &".to_string()));
        assert!(at_rule.is_empty());
        assert_eq!(fill_selector(&selector, "hozo-0"), ".dark .hozo-0");
    }

    #[test]
    fn an_arbitrary_at_rule_wraps_the_whole_rule() {
        let (at_rule, selector) =
            condition_shape(&Condition::ArbitraryAtRule("@supports (display:grid)".to_string()));
        assert_eq!(at_rule, vec!["@supports (display:grid)"]);
        assert_eq!(fill_selector(&selector, "hozo-0"), ".hozo-0");
    }

    #[test]
    fn the_filter_chain_is_ordered_by_function_not_by_how_it_was_written() {
        // Filter functions don't commute: grayscale-then-invert and
        // invert-then-grayscale render differently. Tailwind fixes the
        // order by where each register sits in the value, so writing
        // `invert grayscale` must still come out grayscale-first.
        let props = vec![
            StyleProperty::Filter(FilterFunction::Invert, "invert(100%)".to_string()),
            StyleProperty::Filter(FilterFunction::Grayscale, "grayscale(100%)".to_string()),
            StyleProperty::Filter(FilterFunction::Blur, "blur(8px)".to_string()),
        ];
        let refs: Vec<&StyleProperty> = props.iter().collect();
        assert_eq!(
            filter_value(&refs, false),
            Some("blur(8px) grayscale(100%) invert(100%)".to_string())
        );
        // The element's own chain and the backdrop's are independent.
        assert_eq!(filter_value(&refs, true), None);
    }

    #[test]
    fn a_cleared_filter_slot_drops_out_but_filter_none_clears_everything() {
        let cleared = vec![
            StyleProperty::Filter(FilterFunction::Blur, String::new()),
            StyleProperty::Filter(FilterFunction::Invert, "invert(100%)".to_string()),
        ];
        let refs: Vec<&StyleProperty> = cleared.iter().collect();
        assert_eq!(filter_value(&refs, false), Some("invert(100%)".to_string()));

        let off = vec![
            StyleProperty::Filter(FilterFunction::Invert, "invert(100%)".to_string()),
            StyleProperty::Filter(FilterFunction::None, String::new()),
        ];
        let refs: Vec<&StyleProperty> = off.iter().collect();
        assert_eq!(filter_value(&refs, false), Some("none".to_string()));
    }

    #[test]
    fn known_color_token_resolves_to_real_oklch() {
        let prop = StyleProperty::BackgroundColor(Color::Token("blue-500".to_string()));
        let (name, value) = property_and_value(&prop, &Theme::default());
        assert_eq!(name, "background-color");
        assert_eq!(value, "oklch(62.3% 0.214 259.815)");
    }

    #[test]
    fn unknown_color_token_falls_back_to_a_css_custom_property() {
        let (_, value) =
            property_and_value(&StyleProperty::TextColor(Color::Token("brand-primary".to_string())), &Theme::default());
        assert_eq!(value, "var(--hozo-color-brand-primary)");
    }
}

#[cfg(test)]
mod variant_tests {
    use super::*;

    #[test]
    fn stacked_variants_nest_their_at_rules_and_join_their_selectors() {
        let (at_rules, selector) = condition_shape(&Condition::All(vec![
            Condition::Responsive(hozo_ir::Breakpoint::Md),
            Condition::FirstChild,
            Condition::Hover,
        ]));
        // Written order, outermost first, and `hover:` brings its own.
        assert_eq!(at_rules, vec!["@media (min-width: 768px)", "@media (hover: hover)"]);
        // Suffixes append in written order: `:first-child:hover`, not the
        // reverse. Which is why the fold substitutes the accumulated
        // selector *into* each new template.
        assert_eq!(fill_selector(&selector, "hozo-0"), ".hozo-0:first-child:hover");
    }

    #[test]
    fn hover_carries_the_capability_query() {
        // Not decoration: without it a `:hover` style sticks on a touch
        // device after a tap.
        let (at_rules, selector) = condition_shape(&Condition::Hover);
        assert_eq!(at_rules, vec!["@media (hover: hover)"]);
        assert_eq!(selector, "&:hover");
    }
}
