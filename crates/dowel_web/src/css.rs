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
    Align, BorderStyle, Breakpoint, Color, Condition, ConditionExpr, Dimension, FlexDirection,
    FlexShorthand, Justify, Length, Position, StyleProperty, TextAlign,
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
        StyleProperty::JustifyContent(justify) => (
            "justify-content",
            match justify {
                Justify::Start => "flex-start",
                Justify::Center => "center",
                Justify::End => "flex-end",
                Justify::Between => "space-between",
                Justify::Around => "space-around",
                Justify::Evenly => "space-evenly",
            }
            .to_string(),
        ),
        StyleProperty::Gap(l) => ("gap", length_px(*l)),
        StyleProperty::RowGap(l) => ("row-gap", length_px(*l)),
        StyleProperty::ColumnGap(l) => ("column-gap", length_px(*l)),
        StyleProperty::MarginTop(l) => ("margin-top", length_px(*l)),
        StyleProperty::MarginRight(l) => ("margin-right", length_px(*l)),
        StyleProperty::MarginBottom(l) => ("margin-bottom", length_px(*l)),
        StyleProperty::MarginLeft(l) => ("margin-left", length_px(*l)),
        StyleProperty::PaddingTop(l) => ("padding-top", length_px(*l)),
        StyleProperty::PaddingRight(l) => ("padding-right", length_px(*l)),
        StyleProperty::PaddingBottom(l) => ("padding-bottom", length_px(*l)),
        StyleProperty::PaddingLeft(l) => ("padding-left", length_px(*l)),
        StyleProperty::MarginInlineStart(l) => ("margin-inline-start", length_px(*l)),
        StyleProperty::MarginInlineEnd(l) => ("margin-inline-end", length_px(*l)),
        StyleProperty::PaddingInlineStart(l) => ("padding-inline-start", length_px(*l)),
        StyleProperty::PaddingInlineEnd(l) => ("padding-inline-end", length_px(*l)),
        StyleProperty::Width(d) => ("width", dimension_value(*d)),
        StyleProperty::Height(d) => ("height", dimension_value(*d)),
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
        StyleProperty::BackgroundColor(c) => ("background-color", color_var(c)),
        StyleProperty::Opacity(o) => ("opacity", format!("{o}")),
        StyleProperty::BorderColor(c) => ("border-color", color_var(c)),
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
        StyleProperty::BorderRadius(l) => ("border-radius", length_px(*l)),
        StyleProperty::FontSize(l) => ("font-size", length_px(*l)),
        StyleProperty::FontWeight(w) => ("font-weight", format!("{}", w.0)),
        StyleProperty::LineHeight(l) => ("line-height", length_px(*l)),
        StyleProperty::TextAlign(align) => (
            "text-align",
            match align {
                TextAlign::Left => "left",
                TextAlign::Center => "center",
                TextAlign::Right => "right",
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
        Condition::Expr(expr) => (None, condition_expr_selector(expr)),
    }
}

/// Renders one CSS rule (optionally media-wrapped) for a class + condition
/// group's already-deduped properties.
pub fn render_rule(class_name: &str, condition: &Condition, props: &[StyleProperty]) -> String {
    let (media, suffix) = condition_shape(condition);
    let mut body = String::new();
    for prop in props {
        let (name, value) = property_and_value(prop);
        body.push_str(&format!("  {name}: {value};\n"));
    }
    let rule = format!(".{class_name}{suffix} {{\n{body}}}");
    match media {
        Some(query) => format!("@media {query} {{\n{rule}\n}}"),
        None => rule,
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
