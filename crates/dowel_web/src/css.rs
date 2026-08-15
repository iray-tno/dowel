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
    Align, AlignSelf, BorderStyle, Breakpoint, Color, Condition, ConditionExpr, Dimension, Display,
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
        StyleProperty::InsetTop(l) => ("top", length_px(*l)),
        StyleProperty::InsetRight(l) => ("right", length_px(*l)),
        StyleProperty::InsetBottom(l) => ("bottom", length_px(*l)),
        StyleProperty::InsetLeft(l) => ("left", length_px(*l)),
        StyleProperty::InsetInlineStart(l) => ("inset-inline-start", length_px(*l)),
        StyleProperty::InsetInlineEnd(l) => ("inset-inline-end", length_px(*l)),
        StyleProperty::InsetInline(l) => ("inset-inline", length_px(*l)),
        StyleProperty::InsetBlock(l) => ("inset-block", length_px(*l)),
        StyleProperty::InsetBlockStart(l) => ("inset-block-start", length_px(*l)),
        StyleProperty::InsetBlockEnd(l) => ("inset-block-end", length_px(*l)),
        StyleProperty::BackgroundColor(c) => ("background-color", color_var(c)),
        StyleProperty::Opacity(o) => ("opacity", format!("{o}")),
        StyleProperty::BorderColor(c) => ("border-color", color_var(c)),
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
        StyleProperty::TranslateX(l) => ("translate", format!("{} 0", length_px(*l))),
        StyleProperty::TranslateY(l) => ("translate", format!("0 {}", length_px(*l))),
        StyleProperty::BoxShadow(s) => ("box-shadow", s.clone()),
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
pub fn render_rule(class_name: &str, condition: &Condition, props: &[StyleProperty]) -> String {
    let (media, suffix) = condition_shape(condition);

    // `space-*` targets the element's children rather than the element, so
    // it becomes a second, child-scoped rule instead of a declaration here.
    let (child_props, own_props): (Vec<_>, Vec<_>) =
        props.iter().partition(|p| matches!(p, StyleProperty::SpaceX(_) | StyleProperty::SpaceY(_)));

    let mut rules: Vec<String> = Vec::new();
    if !own_props.is_empty() {
        let mut body = String::new();
        for prop in own_props {
            let (name, value) = property_and_value(prop);
            body.push_str(&format!("  {name}: {value};\n"));
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
