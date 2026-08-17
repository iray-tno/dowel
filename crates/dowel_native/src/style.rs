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
    Align, AlignSelf, BorderStyle, Color, DecorationStyle, Dimension, Display, FilterFunction,
    GradientKind, GradientStop,
    Edge,
    FlexDirection,
    LetterSpacing,
    FlexShorthand, Justify,
    Length, LineHeight, Overflow, Position, Radius, StyleProperty, TextAlign, TextTransform, Theme,
};

fn radius_number(radius: &Radius, theme: &Theme) -> String {
    match radius {
        Radius::Length(l) => number(*l, theme),
        // RN has no infinity. Any radius past half the box's shorter side
        // already renders as a pill, so a large finite value is the
        // standard way to express this -- the approximation is forced here,
        // unlike on Web.
        Radius::Full => "9999".to_string(),
    }
}

/// React Native's `aspectRatio` is a number, where CSS writes a ratio.
fn aspect_number(value: &str) -> Option<String> {
    let (w, h) = value.split_once(" / ")?;
    let (w, h) = (w.parse::<f64>().ok()?, h.parse::<f64>().ok()?);
    Some(format!("{}", w / h))
}

/// A length as a React Native style number, against the project's spacing
/// scale. See `Length::Spacing`.
fn number(length: Length, theme: &Theme) -> String {
    format!("{}", length.px(theme))
}

fn dimension_value(dim: &Dimension, theme: &Theme) -> String {
    match dim {
        Dimension::Length(length) => number(*length, theme),
        Dimension::Percent(pct) => format!("'{pct}%'"),
        Dimension::Auto => "'auto'".to_string(),
        // Refused upstream by `StyleProperty::unsupported_on_native`, which
        // fails the build. Nothing is emitted here so a build that swallowed
        // that error still can't ship a value RN would reject.
        // Also refused upstream: an intrinsic size or a chrome-aware
        // viewport unit has no runtime equivalent to read on this platform.
        Dimension::ViewportWidth(_) | Dimension::ViewportHeight(_) | Dimension::Css(_) => {
            String::new()
        }
    }
}

/// Builds React Native's combined `transform` array from whichever
/// standalone transform properties a rule carries, or `None` if it carries
/// none. Ordered translate -> rotate -> scale, matching how CSS applies its
/// standalone properties, so the two platforms compose identically.
pub fn transform_entry(props: &[StyleProperty], theme: &Theme) -> Option<(&'static str, String)> {
    let mut parts: Vec<String> = Vec::new();
    for prop in props {
        if let StyleProperty::TranslateX(d) = prop {
            parts.push(format!("{{ translateX: {} }}", dimension_value(d, theme)));
        }
    }
    for prop in props {
        if let StyleProperty::TranslateY(d) = prop {
            parts.push(format!("{{ translateY: {} }}", dimension_value(d, theme)));
        }
    }
    for prop in props {
        // An angle that stayed CSS text has no degrees to give. It is
        // refused by name in `StyleProperty::native_gap` rather than
        // dropped here, so leaving it out of the array is the second half
        // of a reported gap, not a silent one.
        if let StyleProperty::Rotate(a) = prop {
            if let Some(degrees) = a.degrees() {
                parts.push(format!("{{ rotate: '{degrees}deg' }}"));
            }
        }
    }
    // React Native has the 3D rotations and the skews as transform entries
    // of their own, in the same order CSS applies them.
    for (name, angle) in [
        ("rotateX", props.iter().find_map(rotate_x)),
        ("rotateY", props.iter().find_map(rotate_y)),
        ("rotateZ", props.iter().find_map(rotate_z)),
        ("skewX", props.iter().find_map(skew_x)),
        ("skewY", props.iter().find_map(skew_y)),
    ] {
        if let Some(degrees) = angle {
            parts.push(format!("{{ {name}: '{degrees}deg' }}"));
        }
    }
    // Scale is a ratio here, not a percentage. RN's public transform type
    // has no scaleZ entry, but its supported 4x4 matrix does. An explicit
    // Z-axis utility carries Scale3d, so split all axes in that case and
    // put Z on the matrix diagonal. Keeping ordinary uniform scale as one
    // `scale` entry avoids applying its Z component twice.
    let axis = |f: fn(&StyleProperty) -> Option<f64>| props.iter().find_map(f);
    let x = axis(scale_x);
    let y = axis(scale_y);
    let z = axis(scale_z);
    let explicit_z = props.iter().any(|p| matches!(p, StyleProperty::Scale3d));
    match (x, y, explicit_z) {
        (Some(x), Some(y), false) if x == y => parts.push(format!("{{ scale: {x} }}")),
        _ => {
            if let Some(x) = x {
                parts.push(format!("{{ scaleX: {x} }}"));
            }
            if let Some(y) = y {
                parts.push(format!("{{ scaleY: {y} }}"));
            }
        }
    }
    if explicit_z {
        if let Some(z) = z {
            parts.push(format!(
                "{{ matrix: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, {z}, 0, 0, 0, 0, 1] }}"
            ));
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(("transform", format!("[{}]", parts.join(", "))))
}

fn rotate_x(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::RotateX(a) => a.degrees(),
        _ => None,
    }
}
fn rotate_y(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::RotateY(a) => a.degrees(),
        _ => None,
    }
}
fn rotate_z(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::RotateZ(a) => a.degrees(),
        _ => None,
    }
}
fn skew_x(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::SkewX(a) => a.degrees(),
        _ => None,
    }
}
fn skew_y(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::SkewY(a) => a.degrees(),
        _ => None,
    }
}
fn scale_x(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::ScaleX(v) => v.ratio(),
        _ => None,
    }
}
fn scale_y(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::ScaleY(v) => v.ratio(),
        _ => None,
    }
}
fn scale_z(prop: &StyleProperty) -> Option<f64> {
    match prop {
        StyleProperty::ScaleZ(v) => v.ratio(),
        _ => None,
    }
}

/// A gradient as React Native's `backgroundImage`, which takes CSS
/// gradient syntax as a string.
///
/// One approximation, and it is visible rather than structural: the
/// interpolation clause is dropped. Tailwind v4 ramps colours through
/// Oklab, so a red-to-blue gradient stays saturated in the middle;
/// React Native's parser doesn't accept `in oklab` and would reject the
/// whole value, so the string it gets interpolates in sRGB and passes
/// through grey. Endpoints are identical, the middle is not.
///
/// Conic gradients are refused by name in
/// `StyleProperty::native_gap` rather than approximated here --
/// `BackgroundImageValue` is `LinearGradientValue | RadialGradientValue`
/// and there is no third one to fall back to.
pub fn background_image_entry(
    props: &[StyleProperty],
    theme: &Theme,
) -> Option<(&'static str, String)> {
    // Last wins between a gradient and `bg-none`, the same scan the Web
    // backend does for the same reason.
    let latest = props.iter().rev().find_map(|p| match p {
        StyleProperty::Gradient(kind, prelude) => Some(Some((*kind, prelude.as_str()))),
        StyleProperty::BackgroundImageNone => Some(None),
        _ => None,
    })?;
    let Some((kind, prelude)) = latest else {
        return Some(("backgroundImage", "'none'".to_string()));
    };
    if kind == GradientKind::Conic {
        return None;
    }
    // Everything up to the interpolation clause: `to right in oklab` is a
    // direction React Native understands followed by one it doesn't.
    let direction = match prelude.find(" in ") {
        Some(cut) => &prelude[..cut],
        None if prelude.starts_with("in ") => "",
        None => prelude,
    };

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
                StyleProperty::GradientStopPosition(s, d) if *s == stop => match d {
                    Dimension::Percent(pct) => Some(format!("{pct}%")),
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or_else(|| stop.default_position().to_string())
    };
    let stop = |s: GradientStop| {
        // `resolve_theme_color` returns a quoted JS string; this one goes
        // inside a larger string, so the quotes come off.
        let color = color(s).unwrap_or_else(|| "'#0000'".to_string());
        format!("{} {}", color.trim_matches('\''), position(s))
    };

    let mut stops = vec![stop(GradientStop::From)];
    if color(GradientStop::Via).is_some() {
        stops.push(stop(GradientStop::Via));
    }
    stops.push(stop(GradientStop::To));

    let args = if direction.is_empty() {
        stops.join(", ")
    } else {
        format!("{direction}, {}", stops.join(", "))
    };
    Some(("backgroundImage", js_string(&format!("{}({args})", kind.css()))))
}

/// Joins whichever filter utilities a rule carries into one `filter`
/// string, in `FilterFunction` order -- the same order the Web backend
/// uses, so the two platforms compose identically. React Native 0.76+
/// accepts the CSS syntax here, so the value text is shared.
pub fn filter_entry(props: &[StyleProperty]) -> Option<(&'static str, String)> {
    let mut functions: Vec<(FilterFunction, &str)> = Vec::new();
    for prop in props {
        let StyleProperty::Filter(function, value) = prop else { continue };
        if *function == FilterFunction::None {
            return Some(("filter", "'none'".to_string()));
        }
        functions.push((*function, value.as_str()));
    }
    if functions.is_empty() {
        return None;
    }
    functions.sort_by_key(|(function, _)| *function);
    let chain: Vec<&str> = functions.iter().map(|(_, v)| *v).filter(|v| !v.is_empty()).collect();
    Some(("filter", format!("'{}'", chain.join(" "))))
}

/// Joins whichever ring/shadow utilities a rule carries into one
/// `boxShadow` string, in the same layer order the Web backend uses so both
/// platforms stack them identically.
///
/// React Native 0.81 accepts a CSS-like string for `boxShadow`, so the
/// composed value is the same shape as the Web one -- only the quoting and
/// the unitless-number convention differ.
pub fn box_shadow_entry(props: &[StyleProperty], theme: &Theme) -> Option<(&'static str, String)> {
    let resolve_color = |color: &Color| resolve_theme_color(color, theme);
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

    let ring_offset = props.iter().find_map(|p| match p {
        StyleProperty::RingOffsetWidth(Length::Px(v)) => Some(*v),
        _ => None,
    });
    let color_of = |find: fn(&StyleProperty) -> Option<&Color>| {
        props.iter().find_map(find).map(|c| resolve_color(c).trim_matches('\'').to_string())
    };
    let shadow_color = color_of(|p| match p {
        StyleProperty::ShadowColor(c) => Some(c),
        _ => None,
    });
    let inset_shadow_color = color_of(|p| match p {
        StyleProperty::InsetShadowColor(c) => Some(c),
        _ => None,
    });

    let mut layers: Vec<String> = Vec::new();
    if let Some(shadow) = props.iter().find_map(|p| match p {
        StyleProperty::InsetShadow(s) => Some(s.clone()),
        _ => None,
    }) {
        layers.push(repaint_shadow(&shadow, inset_shadow_color.as_deref()));
    }
    if let Some(width) = inset_ring {
        layers.push(format!("inset 0 0 0 {width}px {}", paint(inset_ring_color)));
    }
    // Under the ring and pushing it outwards, the same as on Web. The
    // register default is white.
    if let Some(width) = ring_offset {
        layers.push(format!(
            "0 0 0 {width}px {}",
            color_of(|p| match p {
                StyleProperty::RingOffsetColor(c) => Some(c),
                _ => None,
            })
            .unwrap_or_else(|| "#fff".to_string()),
        ));
    }
    if let Some(width) = ring {
        layers.push(format!(
            "0 0 0 {}px {}",
            width + ring_offset.unwrap_or(0.0),
            paint(ring_color)
        ));
    }
    if let Some(shadow) = shadow {
        // See the Web backend: `shadow-none` clears the shadow layer, not
        // the ring beside it.
        if shadow != "none" {
            layers.push(repaint_shadow(&shadow, shadow_color.as_deref()));
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
        Justify::Stretch => "'stretch'",
        Justify::Baseline => "'baseline'",
        // Refused upstream: none of these are in RN's alignment unions.
        Justify::Css(_) => "",
    }
    .to_string()
}

fn border_style_literal(style: &BorderStyle) -> String {
    match style {
        BorderStyle::Solid => "'solid'",
        BorderStyle::Dashed => "'dashed'",
        BorderStyle::Dotted => "'dotted'",
        // Refused upstream by `unsupported_on_native`; nothing valid to emit.
        BorderStyle::Double | BorderStyle::Hidden => "",
        // RN has no 'none' border style; a zero width is how you hide one.
        BorderStyle::None => "'solid'",
    }
    .to_string()
}

/// Resolves against the default Tailwind palette where possible (see module
/// docs); otherwise falls back to a marker string deliberately not
/// real-color-shaped, so a missed resolution fails loudly instead of
/// rendering a plausible-but-wrong color.
fn resolve_theme_color(color: &Color, theme: &Theme) -> String {
    let token = match color {
        Color::Token(token) => token,
        Color::Keyword(keyword) => return js_string(keyword),
        // Written out rather than named, so there is nothing to resolve.
        // Whether React Native can actually read it is a separate question
        // and asked separately -- `native_color_reason` refuses the ones it
        // can't before anything reaches here.
        Color::Css(text) => return js_string(text),
    };
    match theme.color(token) {
        Some(resolved) => js_string(&resolved.hex),
        // Not in the project's theme either. Still deliberately not
        // colour-shaped, so a missed resolution fails loudly rather than
        // rendering something plausible.
        None => format!("'dowel-unresolved:{token}'"),
    }
}

/// Whether a property styles the element's *children* rather than the
/// element itself. These don't belong in the element's own style entry; see
/// `child_property_and_value`.
/// A JavaScript string literal for `value`.
///
/// Not `js_string(value)`: a CSS value can contain the quote
/// character, and a font stack routinely does
/// (`-apple-system, 'Segoe UI', ...`). Wrapping that unescaped produced a
/// generated file that didn't parse -- 76 of them, found by type-checking
/// the output against React Native rather than by anything reading it.
pub fn js_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{escaped}'")
}

/// A placeholder colour as `placeholderTextColor` wants it -- the same
/// resolution every other colour gets, since it is an ordinary colour that
/// happens to travel as a prop.
pub fn placeholder_color(color: &Color, theme: &Theme) -> String {
    resolve_theme_color(color, theme)
}

pub fn is_child_scoped(prop: &StyleProperty) -> bool {
    matches!(
        prop,
        StyleProperty::SpaceX(_)
            | StyleProperty::SpaceY(_)
            | StyleProperty::DivideX(_)
            | StyleProperty::DivideY(_)
            | StyleProperty::DivideColor(_)
            | StyleProperty::DivideStyle(_)
    )
}

/// The style `space-*`/`divide-*` put on each spaced child.
///
/// The Web counterpart is `dowel_web::css::space_declarations`, and the two
/// differ in one place worth naming: Tailwind writes `border-top-style` and
/// `border-bottom-style` as separate longhands so `divide-*-reverse` can
/// flip which edge carries the border. React Native has no per-side border
/// style at all -- only `borderStyle` -- so the style is written once. That
/// is not a loss here: only one edge is given a width, so which edges the
/// style nominally applies to makes no visible difference.
pub fn child_property_and_value(prop: &StyleProperty, theme: &Theme) -> Vec<(&'static str, String)> {
    let resolve_color = |color: &Color| resolve_theme_color(color, theme);
    match prop {
        // Both edges are written, zeroing the leading one, matching what
        // Web emits -- a child with its own margin utility still overrides
        // this, since `DowelSpaced` merges the parent's style behind the
        // child's.
        StyleProperty::SpaceX(l) => vec![
            ("marginInlineStart", "0".to_string()),
            ("marginInlineEnd", dimension_value(l, theme)),
        ],
        StyleProperty::SpaceY(l) => {
            vec![("marginTop", "0".to_string()), ("marginBottom", dimension_value(l, theme))]
        }
        StyleProperty::DivideX(l) => vec![
            ("borderStyle", "'solid'".to_string()),
            // RN spells the logical border widths `borderStartWidth`/
            // `borderEndWidth`; it has no `borderInline*Width`, unlike the
            // margins just above, which do take the CSS logical names.
            ("borderStartWidth", "0".to_string()),
            ("borderEndWidth", dimension_value(l, theme)),
        ],
        StyleProperty::DivideY(l) => vec![
            ("borderStyle", "'solid'".to_string()),
            ("borderTopWidth", "0".to_string()),
            ("borderBottomWidth", dimension_value(l, theme)),
        ],
        StyleProperty::DivideColor(c) => vec![("borderColor", resolve_color(c))],
        StyleProperty::DivideStyle(s) => vec![("borderStyle", border_style_literal(s))],
        _ => Vec::new(),
    }
}

/// Maps one `StyleProperty` to one or more `(rn-style-key, value)` pairs
/// (plural because e.g. `FlexShorthand::Auto` has no single-number RN
/// equivalent and must expand to two keys).
pub fn property_and_value<'a>(prop: &'a StyleProperty, theme: &Theme) -> Vec<(&'a str, String)> {
    let resolve_color = |color: &Color| resolve_theme_color(color, theme);
    match prop {
        // Refused upstream by `unsupported_on_native`, so this is only
        // reached if something bypassed it. Emitting nothing is the safe
        // end of that: a CSS property name means nothing to React Native,
        // and inventing a camelCase spelling would turn a build error into
        // a style that silently does nothing on a device.
        StyleProperty::Arbitrary(..) => Vec::new(),
        // Composed into one `backgroundImage` by `background_image_entry`,
        // the same way the transform axes are.
        StyleProperty::BackgroundImageNone
        | StyleProperty::Gradient(..)
        | StyleProperty::GradientStopColor(..)
        | StyleProperty::GradientStopPosition(..) => Vec::new(),
        StyleProperty::Display(d) => match d {
            Display::Flex => vec![("display", "'flex'".to_string())],
            Display::None => vec![("display", "'none'".to_string())],
            Display::Contents => vec![("display", "'contents'".to_string())],
            // No RN equivalent (Yoga has only the three above). The caller
            // raises `WebOnlyPropertyOnNative` and fails the build; nothing
            // is emitted here so a build that ignored the error can't ship
            // an invalid style value either.
            Display::Block | Display::InlineFlex | Display::Grid | Display::Css(_) => Vec::new(),
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
            // RN's `flex` is a grow factor, and a fraction of the parent is
            // not one -- refused upstream rather than divided into
            // something that would lay out differently.
            FlexShorthand::Fraction(..) => Vec::new(),
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
            Align::Css(_) => "",
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
            AlignSelf::Css(_) => "",
            }
            .to_string(),
        )],
        // `stretch` is legal on alignContent and not on justifyContent, and
        // `baseline` on neither -- so which values are allowed is decided
        // here, per property, rather than inside `justify_literal`, which
        // can't see which of the two it is serving. Both are refused
        // upstream; emitting them anyway would put a value in the
        // StyleSheet that React Native's own types reject.
        StyleProperty::AlignContent(Justify::Baseline) => Vec::new(),
        StyleProperty::JustifyContent(Justify::Stretch | Justify::Baseline) => Vec::new(),
        StyleProperty::AlignContent(justify) => vec![("alignContent", justify_literal(justify))],
        StyleProperty::JustifyContent(justify) => vec![("justifyContent", justify_literal(justify))],
        StyleProperty::Gap(l) => vec![("gap", number(*l, theme))],
        StyleProperty::RowGap(l) => vec![("rowGap", number(*l, theme))],
        StyleProperty::ColumnGap(l) => vec![("columnGap", number(*l, theme))],
        StyleProperty::MarginTop(d) => vec![("marginTop", dimension_value(d, theme))],
        StyleProperty::MarginRight(d) => vec![("marginRight", dimension_value(d, theme))],
        StyleProperty::MarginBottom(d) => vec![("marginBottom", dimension_value(d, theme))],
        StyleProperty::MarginLeft(d) => vec![("marginLeft", dimension_value(d, theme))],
        StyleProperty::PaddingTop(l) => vec![("paddingTop", number(*l, theme))],
        StyleProperty::PaddingRight(l) => vec![("paddingRight", number(*l, theme))],
        StyleProperty::PaddingBottom(l) => vec![("paddingBottom", number(*l, theme))],
        StyleProperty::PaddingLeft(l) => vec![("paddingLeft", number(*l, theme))],
        // RN's own direction-relative props; they resolve against
        // `I18nManager.isRTL` at runtime, same role as CSS's inline-start/end.
        StyleProperty::MarginInlineStart(d) => vec![("marginStart", dimension_value(d, theme))],
        StyleProperty::MarginInlineEnd(d) => vec![("marginEnd", dimension_value(d, theme))],
        StyleProperty::PaddingInlineStart(l) => vec![("paddingStart", number(*l, theme))],
        StyleProperty::PaddingInlineEnd(l) => vec![("paddingEnd", number(*l, theme))],
        StyleProperty::Width(d) => vec![("width", dimension_value(d, theme))],
        StyleProperty::Height(d) => vec![("height", dimension_value(d, theme))],
        StyleProperty::MinWidth(d) => vec![("minWidth", dimension_value(d, theme))],
        StyleProperty::FlexBasis(d) => vec![("flexBasis", dimension_value(d, theme))],
        // The block/inline logical sizes only differ from height/width
        // under a vertical `writing-mode`, which React Native has no
        // concept of -- the same assumption `py-*` already lowers under.
        StyleProperty::BlockSize(d) => vec![("height", dimension_value(d, theme))],
        StyleProperty::InlineSize(d) => vec![("width", dimension_value(d, theme))],
        StyleProperty::MaxBlockSize(d) => vec![("maxHeight", dimension_value(d, theme))],
        StyleProperty::MaxInlineSize(d) => vec![("maxWidth", dimension_value(d, theme))],
        StyleProperty::MinBlockSize(d) => vec![("minHeight", dimension_value(d, theme))],
        StyleProperty::MinInlineSize(d) => vec![("minWidth", dimension_value(d, theme))],
        StyleProperty::MarginBlockStart(d) => vec![("marginTop", dimension_value(d, theme))],
        StyleProperty::MarginBlockEnd(d) => vec![("marginBottom", dimension_value(d, theme))],
        StyleProperty::PaddingBlockStart(l) => vec![("paddingTop", number(*l, theme))],
        StyleProperty::PaddingBlockEnd(l) => vec![("paddingBottom", number(*l, theme))],
        // Refused upstream by `unsupported_on_native`.
        StyleProperty::TranslateZ(_)
        | StyleProperty::TextIndent(_)
        | StyleProperty::BorderSpacingX(_)
        | StyleProperty::BorderSpacingY(_) => Vec::new(),
        StyleProperty::MinHeight(d) => vec![("minHeight", dimension_value(d, theme))],
        StyleProperty::MaxWidth(d) => vec![("maxWidth", dimension_value(d, theme))],
        StyleProperty::MaxHeight(d) => vec![("maxHeight", dimension_value(d, theme))],
        // RN's zIndex is a number; `auto` is refused upstream.
        StyleProperty::ZIndex(z) => z.map_or_else(Vec::new, |z| vec![("zIndex", z.to_string())]),
        // RN's `cursor` takes `auto` and `pointer`; every other keyword is
        // refused upstream by `unsupported_on_native`, so anything reaching
        // here is one of the two.
        // Only the two RN has; the rest are refused upstream, and emitting
        // them anyway would put a value in the StyleSheet that RN's own
        // types reject -- which is the convention every other refused
        // value here already follows.
        StyleProperty::Cursor(keyword) => match keyword.as_str() {
            "auto" | "pointer" => vec![("cursor", js_string(keyword))],
            _ => Vec::new(),
        },
        // RN has `mixBlendMode` and takes the same keywords;
        // `background-blend-mode` is refused upstream.
        // Only the three React Native has; the rest are refused upstream by
        // name. The key is the camelCase of the CSS property, which is how
        // RN spells all three.
        StyleProperty::Keyword("user-select", value) => {
            vec![("userSelect", js_string(value))]
        }
        StyleProperty::Keyword("vertical-align", value) => match *value {
            "auto" | "top" | "bottom" | "middle" => {
                vec![("verticalAlign", js_string(value))]
            }
            _ => Vec::new(),
        },
        StyleProperty::Keyword("transform-origin", value) => {
            vec![("transformOrigin", js_string(value))]
        }
        // The camelCase of the CSS name, which is how RN spells each of
        // these. `font-family` keeps the whole stack: RN takes one family
        // name, but a stack is a legal string there and the platform picks
        // the first it has, which is the same behaviour.
        StyleProperty::Keyword("backface-visibility", v) => {
            vec![("backfaceVisibility", js_string(v))]
        }
        StyleProperty::Keyword("box-sizing", v) => vec![("boxSizing", js_string(v))],
        StyleProperty::Keyword("isolation", v) => vec![("isolation", js_string(v))],
        StyleProperty::Keyword("pointer-events", v) => {
            vec![("pointerEvents", js_string(v))]
        }
        StyleProperty::Keyword("font-style", v) => vec![("fontStyle", js_string(v))],
        StyleProperty::Keyword("font-family", v) => vec![("fontFamily", js_string(v))],
        StyleProperty::Keyword("flex-wrap", v) => vec![("flexWrap", js_string(v))],
        // RN has `objectFit` with the same five keywords, `userSelect`, and
        // `textDecorationLine` with all but `overline`. The per-axis
        // overflows are refused upstream -- RN has only the combined one.
        // RN spells the two inline edges `borderStartWidth`/`borderEndWidth`
        // and has no combined inline/block width; the block edges are top
        // and bottom there, since it has no vertical writing mode. It has
        // no per-edge border style at all -- and needs none, because a
        // width renders on its own.
        StyleProperty::BorderLogicalWidth(edge, l) => match edge {
            Edge::InlineStart => vec![("borderStartWidth", number(*l, theme))],
            Edge::InlineEnd => vec![("borderEndWidth", number(*l, theme))],
            Edge::BlockStart => vec![("borderTopWidth", number(*l, theme))],
            Edge::BlockEnd => vec![("borderBottomWidth", number(*l, theme))],
            Edge::Inline => {
                vec![("borderStartWidth", number(*l, theme)), ("borderEndWidth", number(*l, theme))]
            }
            Edge::Block => {
                vec![("borderTopWidth", number(*l, theme)), ("borderBottomWidth", number(*l, theme))]
            }
            _ => Vec::new(),
        },
        StyleProperty::BorderLogicalStyle(..) => Vec::new(),
        // Absorbed into `numberOfLines` by `truncation_props`, the same way
        // `truncate` is -- React Native clamps lines with a prop, not a
        // style.
        // RN composes every transform into one array, so "off" is the
        // absence of an entry rather than a value; refused upstream so it
        // isn't silently a no-op when written after a transform.
        StyleProperty::RotateNone
        | StyleProperty::ScaleNone
        | StyleProperty::TranslateNone
        | StyleProperty::TransformNone
        | StyleProperty::TransformEmpty
        | StyleProperty::TransformGpu => Vec::new(),
        StyleProperty::LineClamp(_) => Vec::new(),
        StyleProperty::FlexGrow(n) => vec![("flexGrow", format!("{n}"))],
        StyleProperty::FlexShrink(n) => vec![("flexShrink", format!("{n}"))],
        // RN's aspectRatio takes a number, so the ratio is divided out. It
        // has no `auto`, which is refused upstream.
        StyleProperty::AspectRatio(v) => match aspect_number(v) {
            Some(n) => vec![("aspectRatio", n)],
            None => Vec::new(),
        },
        StyleProperty::ObjectFit(v) => vec![("objectFit", js_string(v))],
        StyleProperty::UserSelect(v) => vec![("userSelect", js_string(v))],
        // Underline, line-through and their combination; `overline` is
        // Web-only and refused upstream.
        StyleProperty::TextDecorationLine("overline") => Vec::new(),
        StyleProperty::TextDecorationLine(v) => vec![("textDecorationLine", js_string(v))],
        StyleProperty::OverflowX(_) | StyleProperty::OverflowY(_) => Vec::new(),
        StyleProperty::Keyword(..) => Vec::new(),
        // Every one of these is a vendor-prefixed Web property or a text
        // shaping control React Native doesn't have; refused upstream.
        StyleProperty::KeywordPair(..) => Vec::new(),
        StyleProperty::MixBlendMode(m) => match *m {
            "plus-darker" => Vec::new(),
            _ => vec![("mixBlendMode", js_string(m))],
        },
        StyleProperty::BackgroundBlendMode(_) => Vec::new(),
        // Both refused upstream: Yoga has no flex `order`, and React Native
        // has no multi-column layout.
        StyleProperty::Order(_) | StyleProperty::Columns(_) => Vec::new(),
        StyleProperty::Position(pos) => vec![(
            "position",
            match pos {
                Position::Relative => "'relative'",
                Position::Absolute => "'absolute'",
            Position::Static => "'static'",
            // RN's static is not CSS's; fixed and sticky have no equivalent.
            // All three refused upstream.
            Position::Css(_) => "",
            }
            .to_string(),
        )],
        StyleProperty::InsetTop(d) => vec![("top", dimension_value(d, theme))],
        StyleProperty::InsetRight(d) => vec![("right", dimension_value(d, theme))],
        StyleProperty::InsetBottom(d) => vec![("bottom", dimension_value(d, theme))],
        StyleProperty::InsetLeft(d) => vec![("left", dimension_value(d, theme))],
        StyleProperty::InsetInlineStart(d) => vec![("start", dimension_value(d, theme))],
        StyleProperty::InsetInlineEnd(d) => vec![("end", dimension_value(d, theme))],
        // No axis shorthand in React Native, so both edges are written.
        StyleProperty::InsetInline(d) => {
            vec![("start", dimension_value(d, theme)), ("end", dimension_value(d, theme))]
        }
        StyleProperty::InsetBlock(d) => {
            vec![("top", dimension_value(d, theme)), ("bottom", dimension_value(d, theme))]
        }
        // The block axis is only distinct from top/bottom under a vertical
        // `writing-mode`, which React Native has no concept of.
        StyleProperty::InsetBlockStart(d) => vec![("top", dimension_value(d, theme))],
        StyleProperty::InsetBlockEnd(d) => vec![("bottom", dimension_value(d, theme))],
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
        StyleProperty::BorderTopWidth(l) => vec![("borderTopWidth", number(*l, theme))],
        StyleProperty::BorderRightWidth(l) => vec![("borderRightWidth", number(*l, theme))],
        StyleProperty::BorderBottomWidth(l) => vec![("borderBottomWidth", number(*l, theme))],
        StyleProperty::BorderLeftWidth(l) => vec![("borderLeftWidth", number(*l, theme))],
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
                Radius::Length(l) => number(*l, theme),
                // RN has no infinity. Any radius past half the box's
                // shorter side already renders as a pill, so a large
                // finite value is the standard way to express this -- the
                // approximation is forced here, unlike on Web.
                Radius::Full => "9999".to_string(),
            },
        )],
        StyleProperty::BorderTopLeftRadius(r) => vec![("borderTopLeftRadius", radius_number(r, theme))],
        StyleProperty::BorderTopRightRadius(r) => vec![("borderTopRightRadius", radius_number(r, theme))],
        StyleProperty::BorderBottomRightRadius(r) => {
            vec![("borderBottomRightRadius", radius_number(r, theme))]
        }
        StyleProperty::BorderBottomLeftRadius(r) => {
            vec![("borderBottomLeftRadius", radius_number(r, theme))]
        }
        // React Native has the logical corner names too, so `rounded-s-*`
        // stays correct under RTL rather than being flattened to left/right.
        StyleProperty::BorderStartStartRadius(r) => {
            vec![("borderStartStartRadius", radius_number(r, theme))]
        }
        StyleProperty::BorderStartEndRadius(r) => vec![("borderStartEndRadius", radius_number(r, theme))],
        StyleProperty::BorderEndStartRadius(r) => vec![("borderEndStartRadius", radius_number(r, theme))],
        StyleProperty::BorderEndEndRadius(r) => vec![("borderEndEndRadius", radius_number(r, theme))],
        StyleProperty::FontSize(l) => vec![("fontSize", number(*l, theme))],
        // RN's `fontWeight` type is a *string* ('100'..'900'/'normal'/
        // 'bold'), not a number -- unlike CSS's numeric font-weight.
        StyleProperty::FontWeight(w) => vec![("fontWeight", format!("'{}'", w.0))],
        // Both of these are absolute in React Native. A font-relative form
        // reaching here means `fold_font_relative` found no font size on the
        // element to resolve it against, and it was refused by name there --
        // emitting nothing keeps the object valid if that error is ignored.
        StyleProperty::LineHeight(lh) => match lh {
            LineHeight::Length(l) => vec![("lineHeight", number(*l, theme))],
            LineHeight::Ratio(_) => Vec::new(),
        },
        StyleProperty::LetterSpacing(ls) => match ls {
            LetterSpacing::Px(l) => vec![("letterSpacing", number(*l, theme))],
            LetterSpacing::Em(_) => Vec::new(),
        },
        StyleProperty::Overflow(o) => vec![(
            "overflow",
            match o {
                Overflow::Visible => "'visible'",
                Overflow::Hidden => "'hidden'",
                Overflow::Scroll => "'scroll'",
                // Refused upstream: RN's overflow has no auto or clip.
                Overflow::Css(_) => "",
            }
            .to_string(),
        )],
        // RN Text wraps by default, so `normal` is genuinely a no-op there.
        // `nowrap` is refused upstream -- suppressing wrapping needs the
        // `numberOfLines` prop, not a style.
        StyleProperty::WhiteSpace(_) => Vec::new(),
        // All refused upstream by `unsupported_on_native`.
        | StyleProperty::TextOverflow(_)
        | StyleProperty::GridTemplateColumns(_)
        | StyleProperty::GridTemplateRows(_)
        | StyleProperty::GridColumnStart(_)
        | StyleProperty::GridColumnEnd(_)
        | StyleProperty::GridRowStart(_)
        | StyleProperty::GridRowEnd(_)
        | StyleProperty::GridColumn(_)
        | StyleProperty::GridRow(_)
        | StyleProperty::TransitionProperty(_)
        | StyleProperty::TransitionDuration(_)
        | StyleProperty::TransitionTimingFunction(_)
        | StyleProperty::Animation(_)
        // Child-scoped: these mean something for the element's children, not
        // for the element, and are routed to `child_property_and_value`
        // before they reach here. Nothing to emit on the element itself.
        | StyleProperty::SpaceX(_)
        | StyleProperty::SpaceY(_)
        | StyleProperty::DivideX(_)
        | StyleProperty::DivideY(_)
        | StyleProperty::DivideColor(_)
        | StyleProperty::DivideStyle(_)
        // No React Native equivalent at all; each refused by name upstream.
        | StyleProperty::Fill(_)
        | StyleProperty::Stroke(_)
        | StyleProperty::StrokeWidth(_)
        | StyleProperty::AccentColor(_)
        | StyleProperty::CaretColor(_)
        | StyleProperty::PlaceholderColor(_)
        | StyleProperty::TextDecorationThickness(_)
        // Refused upstream: RN has no text-decoration metrics at all.
        | StyleProperty::TextUnderlineOffset(_)
        | StyleProperty::ScrollMargin(..)
        | StyleProperty::ScrollPadding(..)
        | StyleProperty::ScrollBehavior(_)
        | StyleProperty::MaskClip(_)
        | StyleProperty::MaskOrigin(_)
        | StyleProperty::MaskMode(_)
        | StyleProperty::MaskType(_)
        | StyleProperty::MaskSize(_)
        | StyleProperty::MaskPosition(_)
        | StyleProperty::MaskRepeat(_)
        | StyleProperty::MaskImageNone
        | StyleProperty::MaskStopColor(..)
        | StyleProperty::MaskStopPosition(..)
        | StyleProperty::MaskSlotArgument(..)
        | StyleProperty::MaskRadialShape(_)
        | StyleProperty::MaskRadialSize(_)
        | StyleProperty::MaskRadialPosition(_)
        | StyleProperty::MaskComposite(_)
        | StyleProperty::ScrollbarWidth(_)
        | StyleProperty::ScrollbarGutter(_)
        | StyleProperty::ScrollbarThumbColor(_)
        | StyleProperty::ScrollbarTrackColor(_) => Vec::new(),
        StyleProperty::TextDecorationColor(c) => vec![("textDecorationColor", resolve_color(c))],
        // React Native's `textDecorationStyle` takes the same five values CSS
        // does. `wavy` was refused here until the refusal audit checked the
        // claim against RN's own types and found it false -- the whole union
        // is accepted, and the platform caveat (these render on iOS and are
        // ignored on Android) applies equally to `dotted` and `dashed`, which
        // were never refused.
        StyleProperty::TextDecorationStyle(s) => vec![(
            "textDecorationStyle",
            match s {
                DecorationStyle::Solid => "'solid'",
                DecorationStyle::Double => "'double'",
                DecorationStyle::Dotted => "'dotted'",
                DecorationStyle::Dashed => "'dashed'",
                DecorationStyle::Wavy => "'wavy'",
            }
            .to_string(),
        )],
        StyleProperty::OutlineWidth(l) => vec![("outlineWidth", number(*l, theme))],
        // RN's `outlineStyle` accepts only solid/dotted/dashed -- verified
        // against react-native-css's own parser, which warns on anything
        // else. So `outline-none` is expressed the way a border is hidden:
        // zero width. Reusing `border_style_literal` here would emit
        // `'solid'`, which is the opposite of what was asked for.
        StyleProperty::OutlineStyle(BorderStyle::None) => {
            vec![("outlineWidth", "0".to_string())]
        }
        StyleProperty::OutlineStyle(s) => vec![("outlineStyle", border_style_literal(s))],
        StyleProperty::OutlineColor(c) => vec![("outlineColor", resolve_color(c))],
        StyleProperty::OutlineOffset(l) => vec![("outlineOffset", number(*l, theme))],
        StyleProperty::TextAlign(align) => vec![(
            "textAlign",
            match align {
                TextAlign::Left => "'left'",
                TextAlign::Center => "'center'",
                TextAlign::Right => "'right'",
                TextAlign::Css(v) => return vec![("textAlign", js_string(v))],
            }
            .to_string(),
        )],
        // Composed into a single `transform` by the caller, since RN has no
        // standalone rotate/scale/translate -- see `transform_entry`.
        StyleProperty::Rotate(_)
        | StyleProperty::ScaleX(_)
        | StyleProperty::ScaleY(_)
        | StyleProperty::ScaleZ(_)
        | StyleProperty::Scale3d
        | StyleProperty::RotateX(_)
        | StyleProperty::RotateY(_)
        | StyleProperty::RotateZ(_)
        | StyleProperty::SkewX(_)
        | StyleProperty::SkewY(_)
        | StyleProperty::TranslateX(_)
        | StyleProperty::TranslateY(_) => Vec::new(),
        // RN accepts a string for both, so the CSS text carries over as-is.
        // Composed with any ring layers by `box_shadow_entry`, not emitted
        // here -- `style_pairs` filters these out before this runs.
        StyleProperty::BoxShadow(_)
        | StyleProperty::RingWidth(_)
        | StyleProperty::RingColor(_)
        | StyleProperty::InsetRingWidth(_)
        | StyleProperty::RingOffsetWidth(_)
        | StyleProperty::RingOffsetColor(_)
        | StyleProperty::ShadowColor(_)
        | StyleProperty::InsetShadowColor(_)
        | StyleProperty::InsetRingColor(_)
        | StyleProperty::InsetShadow(_) => vec![],
        // Composed, not emitted here -- see `filter_entry`. `BackdropFilter`
        // is refused upstream: React Native has no such style key.
        StyleProperty::Filter(..) | StyleProperty::BackdropFilter(..) => vec![],
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
        assert_eq!(property_and_value(&StyleProperty::PaddingTop(Length::Px(24.0)), &Theme::default()), vec![("paddingTop", "24".to_string())]);
    }

    #[test]
    fn flex_grow_is_a_bare_number() {
        assert_eq!(
            property_and_value(&StyleProperty::Flex(FlexShorthand::Grow(1.0)), &Theme::default()),
            vec![("flex", "1".to_string())]
        );
    }

    #[test]
    fn flex_auto_expands_to_two_keys() {
        assert_eq!(
            property_and_value(&StyleProperty::Flex(FlexShorthand::Auto), &Theme::default()),
            vec![("flexGrow", "1".to_string()), ("flexShrink", "1".to_string())]
        );
    }

    #[test]
    fn font_weight_is_a_string() {
        assert_eq!(
            property_and_value(&StyleProperty::FontWeight(dowel_ir::FontWeight(700)), &Theme::default()),
            vec![("fontWeight", "'700'".to_string())]
        );
    }

    #[test]
    fn known_color_token_resolves_to_real_hex() {
        assert_eq!(
            property_and_value(&StyleProperty::BackgroundColor(Color::Token("blue-500".to_string())), &Theme::default()),
            vec![("backgroundColor", "'#2b7fff'".to_string())]
        );
    }

    #[test]
    fn unknown_color_token_falls_back_to_a_marker_string() {
        assert_eq!(
            property_and_value(&StyleProperty::TextColor(Color::Token("brand-primary".to_string())), &Theme::default()),
            vec![("color", "'dowel-unresolved:brand-primary'".to_string())]
        );
    }
}

/// Repaints a shadow's layers in `color`. The Web backend's `repaint_shadow`
/// with the same reasoning and the same cut: the colour is the tail of each
/// layer, and the shadow table it reads from is in this repository.
fn repaint_shadow(shadow: &str, color: Option<&str>) -> String {
    let Some(paint) = color else { return shadow.to_string() };
    shadow
        .split(',')
        .map(|layer| match layer.rfind("rgb(") {
            Some(cut) => format!("{}{paint}", &layer[..cut]),
            None => layer.trim().to_string(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}
