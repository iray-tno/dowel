//! Dowel IR to React Native primitive/StyleSheet lowering (Native backend).
//!
//! `Condition::Always` merges directly into the rendered `style` prop.
//! Other conditions merge too, each keyed to whatever real value drives
//! them -- `Disabled` uses `PropSet.disabled`'s guard (the *style*
//! condition itself carries no expression; the actual boolean comes from
//! the separate `disabled={...}` prop), `Expr` carries its own guard
//! directly. Both get spliced into a conditional `style={[base, guard &&
//! variant]}` array, re-emitting the guard verbatim from `source` (see
//! `render_condition_expr`) exactly like `dowel_web` does for its
//! attribute-toggle wiring -- same "never evaluate, only re-emit" rule.
//!
//! `Pressed` merges too, but differently: RN's `Pressable` already tracks
//! press state natively via a `style={({ pressed }) => [...]}` render-prop
//! form (no synthesized state needed, unlike what an earlier pass of this
//! design assumed) -- so a node with a `Pressed` condition gets its whole
//! `style` prop wrapped in that function instead of being a plain array.
//! Only applies when `component == "Pressable"` (Button maps to it too);
//! a function isn't a valid `style` value on View/Text, so `Pressed` stays
//! unmerged there, same treatment as Hover/Focus/Responsive below.
//!
//! `Hover`/`Focus`/`Responsive` still don't merge into anything: no native
//! mobile-touch hover, and RN focus/window-dimension tracking are real but
//! separate mechanisms this pass doesn't build. Their style objects are
//! still computed (nothing is lost), just unused in the render -- the one
//! remaining honest gap, not a silent one.

mod markup;
mod style;

use dowel_ir::{
    Breakpoint, Condition, ConditionExpr, Diagnostic, DiagnosticCode, ExprRef, Node,
    Severity, StyleProperty, TextContent,
};

pub struct LowerOutput {
    pub jsx: String,
    /// A `StyleSheet.create({ ... })`-ready JS object literal (without the
    /// `StyleSheet.create(` wrapper -- left to the caller, since whether/how
    /// to wrap and import `StyleSheet` is a codegen-site decision).
    pub styles: String,
    pub diagnostics: Vec<Diagnostic>,
}

struct NameAllocator {
    next: u32,
}

impl NameAllocator {
    fn alloc(&mut self) -> String {
        let name = format!("dowel{}", self.next);
        self.next += 1;
        name
    }
}

/// `source` is the original TSX text `root` was parsed from -- needed to
/// re-emit `ExprRef`/`ConditionExpr` guards verbatim (they're spans into
/// it, never evaluated by the compiler; see `dowel_ir`'s doc comments).
pub fn lower(root: &Node, source: &str) -> LowerOutput {
    let mut allocator = NameAllocator { next: 0 };
    let mut style_entries: Vec<(String, Vec<StyleProperty>)> = Vec::new();
    let mut diagnostics = Vec::new();

    let jsx = render_node(root, source, &mut allocator, &mut style_entries, &mut diagnostics);

    let mut styles = String::from("{\n");
    for (name, props) in &style_entries {
        styles.push_str(&format!("  {name}: {{\n"));
        // Distinct IR properties can collapse onto one RN key (all four
        // per-side border styles map to `borderStyle`), which would emit a
        // duplicate object key. Keep the last, matching how JS itself would
        // resolve it -- but written once.
        let mut emitted: Vec<(&'static str, String)> = Vec::new();
        for prop in props {
            for (key, value) in style::property_and_value(prop) {
                // A property refused for Native (see
                // `StyleProperty::unsupported_on_native`) yields no value;
                // writing the key anyway would emit `height: ,`, which isn't
                // even parseable JS.
                if value.is_empty() {
                    continue;
                }
                match emitted.iter_mut().find(|(existing, _)| *existing == key) {
                    Some(slot) => slot.1 = value,
                    None => emitted.push((key, value)),
                }
            }
        }
        if let Some(transform) = style::transform_entry(props) {
            emitted.push(transform);
        }
        for (key, value) in emitted {
            styles.push_str(&format!("    {key}: {value},\n"));
        }
        styles.push_str("  },\n");
    }
    styles.push('}');

    LowerOutput { jsx, styles, diagnostics }
}

/// Byte-slices `source` at an `ExprRef`'s span. Spans come from oxc's own
/// tokenizer over this same `source`, so they're always on UTF-8 character
/// boundaries -- not re-validated here.
fn source_text(source: &str, expr_ref: ExprRef) -> &str {
    &source[expr_ref.0.start as usize..expr_ref.0.end as usize]
}

/// Re-emits a `ConditionExpr` as a JS boolean expression by splicing the
/// original source at each leaf `Ref`'s span, reconstructed with real
/// `&&`/`||`/`!` matching the combinator structure the compiler built
/// (see dowel_parser's `dynamic_class` module) -- never anything parsed
/// out of the leaves themselves.
fn render_condition_expr(source: &str, expr: &ConditionExpr) -> String {
    match expr {
        ConditionExpr::Ref(r) => source_text(source, *r).to_string(),
        ConditionExpr::Not(inner) => format!("!({})", render_condition_expr(source, inner)),
        ConditionExpr::And(a, b) => {
            format!("({}) && ({})", render_condition_expr(source, a), render_condition_expr(source, b))
        }
        ConditionExpr::Or(a, b) => {
            format!("({}) || ({})", render_condition_expr(source, a), render_condition_expr(source, b))
        }
    }
}

fn render_node(
    node: &Node,
    source: &str,
    allocator: &mut NameAllocator,
    style_entries: &mut Vec<(String, Vec<StyleProperty>)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let base_name = allocator.alloc();
    let mut style_array_parts: Vec<String> = Vec::new();
    // Held separately from `style_array_parts` because they can only be
    // merged once `component` is known (below) -- RN's pressed-render-prop
    // form of `style` only exists on Pressable; on View/Text a function
    // isn't a valid style value at all, so it must not be used there.
    let mut pressed_parts: Vec<String> = Vec::new();

    for declaration in &node.style {
        // Refused rather than dropped: silently ignoring a `block`/`grid`/
        // `h-screen` would leave a layout that looks right on Web and is
        // wrong on device with nothing pointing at the cause.
        if let Some(reason) = declaration.property.unsupported_on_native() {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::WebOnlyPropertyOnNative,
                severity: Severity::Error,
                message: format!("{reason} -- this utility is Web-only."),
                span: node.span,
            });
        }
    }

    for (condition, props) in dowel_ir::group_by_condition(&node.style) {
        let props = dowel_ir::dedupe_last_wins(props);
        if props.is_empty() {
            continue;
        }
        let name = match condition_suffix(&condition) {
            None => base_name.clone(),
            Some(suffix) => format!("{base_name}_{suffix}"),
        };
        match &condition {
            Condition::Always => style_array_parts.push(format!("styles.{name}")),
            Condition::Disabled => {
                if let Some(disabled) = &node.props.disabled {
                    let guard = render_condition_expr(source, disabled);
                    style_array_parts.push(format!("({guard}) && styles.{name}"));
                }
                // No `disabled` prop on this node -- nothing drives this
                // condition, so it's computed but left unmerged.
            }
            Condition::Pressed => pressed_parts.push(format!("pressed && styles.{name}")),
            Condition::Expr(expr) => {
                let guard = render_condition_expr(source, expr);
                style_array_parts.push(format!("({guard}) && styles.{name}"));
            }
            Condition::Hover
            | Condition::Focus
            | Condition::Responsive(_)
            // `Dark` and `FirstChild` both have real RN counterparts in
            // principle -- `useColorScheme()` and the child's index -- but
            // neither is a style condition, so wiring them needs machinery
            // this pass doesn't build.
            | Condition::Dark
            | Condition::FirstChild => {
                // No RN mechanism yet (see module docs) -- computed, not merged.
            }
        }
        style_entries.push((name, props));
    }

    let (component, extra_props) = markup::native_component(node, diagnostics);

    let needs_pressed_fn = component == "Pressable" && !pressed_parts.is_empty();
    if needs_pressed_fn {
        style_array_parts.extend(pressed_parts);
    }

    let mut props_text = String::new();
    if needs_pressed_fn {
        props_text.push_str(&format!(" style={{({{ pressed }}) => [{}]}}", style_array_parts.join(", ")));
    } else if style_array_parts.len() == 1 && !style_array_parts[0].contains("&&") {
        props_text.push_str(&format!(" style={{{}}}", style_array_parts[0]));
    } else if !style_array_parts.is_empty() {
        props_text.push_str(&format!(" style={{[{}]}}", style_array_parts.join(", ")));
    }
    for (key, value) in &extra_props {
        props_text.push_str(&format!(r#" {key}="{value}""#));
    }
    if let Some(on_press) = node.props.on_press {
        props_text.push_str(&format!(" onPress={{{}}}", source_text(source, on_press)));
    }
    if let Some(disabled) = &node.props.disabled {
        props_text.push_str(&format!(" disabled={{{}}}", render_condition_expr(source, disabled)));
    }
    // Everything Dowel doesn't model, re-emitted verbatim and last so JSX's
    // last-wins duplicate resolution keeps matching the source's own
    // ordering semantics.
    for prop in &node.props.passthrough {
        props_text.push(' ');
        props_text.push_str(source_text(source, prop.span));
    }

    let inner = match &node.text {
        Some(TextContent::Literal(text)) => escape_jsx_text(text),
        Some(TextContent::Dynamic(_)) | None => node
            .children
            .iter()
            .map(|child| render_node(child, source, allocator, style_entries, diagnostics))
            .collect(),
    };

    format!("<{component}{props_text}>{inner}</{component}>")
}

fn escape_jsx_text(text: &str) -> String {
    text.replace('{', "&#123;").replace('}', "&#125;")
}

/// `None` for `Always` (uses the node's base style name directly);
/// otherwise a name-safe suffix identifying the condition.
fn condition_suffix(condition: &Condition) -> Option<String> {
    match condition {
        Condition::Always => None,
        Condition::Hover => Some("hover".to_string()),
        Condition::Focus => Some("focus".to_string()),
        Condition::Disabled => Some("disabled".to_string()),
        Condition::Pressed => Some("pressed".to_string()),
        Condition::Dark => Some("dark".to_string()),
        Condition::FirstChild => Some("first".to_string()),
        Condition::Responsive(bp) => Some(
            match bp {
                Breakpoint::Sm => "sm",
                Breakpoint::Md => "md",
                Breakpoint::Lg => "lg",
                Breakpoint::Xl => "xl",
                Breakpoint::Xl2 => "xl2",
            }
            .to_string(),
        ),
        Condition::Expr(expr) => {
            let mut refs = Vec::new();
            collect_expr_refs(expr, &mut refs);
            Some(format!(
                "cond_{}",
                refs.iter().map(|r: &ExprRef| format!("{}_{}", r.0.start, r.0.end)).collect::<Vec<_>>().join("_")
            ))
        }
    }
}

fn collect_expr_refs(expr: &ConditionExpr, out: &mut Vec<ExprRef>) {
    match expr {
        ConditionExpr::Ref(r) => out.push(*r),
        ConditionExpr::Not(inner) => collect_expr_refs(inner, out),
        ConditionExpr::And(a, b) | ConditionExpr::Or(a, b) => {
            collect_expr_refs(a, out);
            collect_expr_refs(b, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOGIN_EXAMPLE: &str = r#"
import { View, Text, Button } from '@dowel/core'

export function Login() {
  return (
    <View className="flex-1 items-center justify-center p-6">
      <Text className="text-xl font-bold">
        Welcome
      </Text>

      <Button className="mt-4 px-4 py-2">
        Continue
      </Button>
    </View>
  )
}
"#;

    #[test]
    fn lowers_the_login_example_to_rn_jsx_and_styles() {
        let parsed = dowel_parser::parse_tsx(LOGIN_EXAMPLE);
        let root = &parsed.roots[0];
        let output = lower(root, LOGIN_EXAMPLE);

        assert!(output.jsx.starts_with("<View style={styles.dowel0}>"));
        assert!(output.jsx.contains("<Text style={styles.dowel1}>Welcome</Text>"));
        assert!(output.jsx.contains(r#"<Pressable style={styles.dowel2} accessibilityRole="button">Continue</Pressable>"#));

        assert!(output.styles.contains("dowel0: {"));
        assert!(output.styles.contains("flex: 1,"));
        assert!(output.styles.contains("paddingTop: 24,"));
        assert!(output.styles.contains("dowel1: {"));
        assert!(output.styles.contains("fontSize: 20,"));
        assert!(output.styles.contains("fontWeight: '700',"));
        assert!(output.styles.contains("dowel2: {"));
        // `px-4` is Tailwind's logical inline axis, so this lowers to RN's
        // direction-relative props rather than paddingLeft/paddingRight.
        assert!(output.styles.contains("paddingStart: 16,"));
        assert!(output.styles.contains("paddingEnd: 16,"));
        // No `px`/CSS units anywhere -- these are unitless RN numbers.
        assert!(!output.styles.contains("px"));

        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn disabled_condition_merges_into_a_conditional_style_array_when_a_disabled_prop_exists() {
        let source = r#"
            import { Button } from '@dowel/core'
            const el = <Button disabled={isLoading} className="p-2 disabled:opacity-50">Save</Button>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.styles.contains("dowel0_disabled: {"));
        assert!(output.styles.contains("opacity: 0.5,"));
        assert!(output.jsx.contains("style={[styles.dowel0, (isLoading) && styles.dowel0_disabled]}"));
        assert!(output.jsx.contains("disabled={isLoading}"));
    }

    #[test]
    fn unmodeled_props_and_spreads_reach_the_output() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-4" {...rest} onLayout={onLayout} testID="row" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.jsx.contains("{...rest}"));
        assert!(output.jsx.contains("onLayout={onLayout}"));
        assert!(output.jsx.contains(r#"testID="row""#));
    }

    #[test]
    fn pressed_condition_wraps_style_in_rn_pressable_render_prop() {
        let source = r#"
            import { Button } from '@dowel/core'
            const el = <Button className="p-2 pressed:opacity-50">Save</Button>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.styles.contains("dowel0_pressed: {"));
        assert!(output.styles.contains("opacity: 0.5,"));
        assert!(output.jsx.contains("style={({ pressed }) => [styles.dowel0, pressed && styles.dowel0_pressed]}"));
    }

    #[test]
    fn pressed_condition_stays_unmerged_on_view_since_style_cannot_be_a_function_there() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-2 pressed:opacity-50" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.styles.contains("dowel0_pressed: {"));
        assert!(output.jsx.contains("style={styles.dowel0}"));
        assert!(!output.jsx.contains("pressed"));
    }

    #[test]
    fn disabled_condition_stays_unmerged_without_a_disabled_prop() {
        // Nothing drives "disabled-ness" here -- the className has a
        // disabled: variant but the component never actually received a
        // `disabled` prop, so there's no guard to merge with. Computed,
        // not silently dropped, but also not merged into anything.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="disabled:opacity-50" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.styles.contains("dowel0_disabled: {"));
        assert!(!output.jsx.contains("dowel0_disabled"));
    }

    #[test]
    fn dynamic_class_name_guard_merges_into_the_style_array() {
        let source = r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', active && 'text-xl')} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.jsx.contains("style={[styles.dowel0, (active) && styles.dowel0_cond_"));
    }

    #[test]
    fn hover_and_focus_still_do_not_merge_into_anything() {
        // No RN mechanism for either (see module docs) -- still computed,
        // still not merged, unlike Disabled/Expr which now are.
        let node = dowel_ir::Node {
            primitive: dowel_ir::Primitive::View,
            style: vec![
                dowel_ir::StyleDeclaration {
                    property: dowel_ir::StyleProperty::Opacity(1.0),
                    condition: dowel_ir::Condition::Always,
                },
                dowel_ir::StyleDeclaration {
                    property: dowel_ir::StyleProperty::Opacity(0.5),
                    condition: dowel_ir::Condition::Hover,
                },
            ],
            props: dowel_ir::PropSet::default(),
            children: Vec::new(),
            text: None,
            class_name_fallback: Vec::new(),
            span: dowel_ir::SourceSpan { start: 0, end: 0 },
        };
        let output = lower(&node, "");
        assert!(output.jsx.contains("style={styles.dowel0}"));
        assert!(output.styles.contains("dowel0_hover: {"));
        assert!(!output.jsx.contains("dowel0_hover"));
    }

    #[test]
    fn transforms_compose_into_rn_single_transform_array() {
        // RN has no standalone rotate/scale/translate, so several IR
        // properties collapse into one entry -- ordered translate, rotate,
        // scale to match how CSS applies its standalone equivalents.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="scale-95 rotate-45 translate-x-2" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.styles.contains(
            "transform: [{ translateX: 8 }, { rotate: '45deg' }, { scale: 0.95 }],"
        ));
    }

    #[test]
    fn shadow_and_filter_carry_across_as_strings() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="shadow-lg blur-sm" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.styles.contains("boxShadow: '0 10px 15px -3px"));
        assert!(output.styles.contains("filter: 'blur(8px)',"));
    }

    #[test]
    fn web_only_display_is_refused_rather_than_dropped() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="block" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::WebOnlyPropertyOnNative);
        assert_eq!(output.diagnostics[0].severity, dowel_ir::Severity::Error);
        // And nothing is emitted for it, so a build that ignored the error
        // still can't produce an invalid RN style value.
        assert!(!output.styles.contains("display"));
    }

    #[test]
    fn viewport_height_is_refused_and_leaves_valid_output() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="h-screen" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::WebOnlyPropertyOnNative);
        assert_eq!(output.diagnostics[0].severity, dowel_ir::Severity::Error);
        // The key must be dropped entirely, not written with an empty
        // value -- `height: ,` isn't parseable JS.
        assert!(!output.styles.contains("height"));
        assert!(!output.styles.contains(": ,"));
    }

    #[test]
    fn portable_display_values_lower_normally() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="hidden" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.diagnostics.is_empty());
        assert!(output.styles.contains("display: 'none',"));
    }

    #[test]
    fn interactive_pressable_without_role_is_diagnosed_from_real_source() {
        // As with dowel_web: previously only reachable by hand-constructing
        // a `Node` -- the parser didn't populate on_press/accessibility_role
        // at all until dowel_parser::jsx gained that attribute parsing.
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable onPress={handleTap}>Tap</Pressable>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::A11yInteractiveWithoutRole);
        assert!(output.jsx.contains("onPress={handleTap}"));

        let source_with_role = r#"
            import { Pressable } from '@dowel/core'
            const el = (
              <Pressable onPress={handleTap} accessibilityRole="button">Tap</Pressable>
            )
            "#;
        let parsed_with_role = dowel_parser::parse_tsx(source_with_role);
        let output_with_role = lower(&parsed_with_role.roots[0], source_with_role);
        assert!(output_with_role.diagnostics.is_empty());
        assert!(output_with_role.jsx.contains(r#"accessibilityRole="button""#));
    }
}
