//! Dowel IR to React Native primitive/StyleSheet lowering (Native backend).
//!
//! Phase 0 scope, matching the same bar `dowel_web` was held to before its
//! own napi/Vite wiring: prove the *structural* mapping (primitive ->
//! RN component, StyleProperty -> RN style key/value, Condition -> a named
//! style entry) is correct. Only `Condition::Always` gets wired into the
//! rendered `style={...}` prop -- non-Always conditions (Hover/Focus/
//! Disabled/Responsive/Expr) still get a correctly computed style object,
//! but merging it into a live `style={[base, cond && variant]}` array needs
//! `PropSet.disabled`/`on_press` to actually be populated from JSX (they
//! aren't yet -- dowel_parser doesn't parse those attributes) and, for
//! Expr, the same "re-emit the guard verbatim" runtime wiring `dowel_web`
//! also still owes. Tracked as a known gap, not silently skipped.
//!
//! `Hover`/`Focus` compile fine structurally here too, even though neither
//! has a native mobile-touch equivalent (no hover on touch; RN focus is a
//! real but separate mechanism via onFocus/onBlur) -- Phase 0 doesn't
//! block on deciding what to do with them, it just doesn't lose the data.

mod markup;
mod style;

use dowel_ir::{Breakpoint, Condition, ConditionExpr, Diagnostic, ExprRef, Node, StyleProperty, TextContent};

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

pub fn lower(root: &Node) -> LowerOutput {
    let mut allocator = NameAllocator { next: 0 };
    let mut style_entries: Vec<(String, Vec<StyleProperty>)> = Vec::new();
    let mut diagnostics = Vec::new();

    let jsx = render_node(root, &mut allocator, &mut style_entries, &mut diagnostics);

    let mut styles = String::from("{\n");
    for (name, props) in &style_entries {
        styles.push_str(&format!("  {name}: {{\n"));
        for prop in props {
            for (key, value) in style::property_and_value(prop) {
                styles.push_str(&format!("    {key}: {value},\n"));
            }
        }
        styles.push_str("  },\n");
    }
    styles.push('}');

    LowerOutput { jsx, styles, diagnostics }
}

fn render_node(
    node: &Node,
    allocator: &mut NameAllocator,
    style_entries: &mut Vec<(String, Vec<StyleProperty>)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let base_name = allocator.alloc();
    let mut base_style_ref = None;

    for (condition, props) in dowel_ir::group_by_condition(&node.style) {
        let props = dowel_ir::dedupe_last_wins(props);
        if props.is_empty() {
            continue;
        }
        let name = match condition_suffix(&condition) {
            None => base_name.clone(),
            Some(suffix) => format!("{base_name}_{suffix}"),
        };
        if condition == Condition::Always {
            base_style_ref = Some(name.clone());
        }
        style_entries.push((name, props));
    }

    let (component, extra_props) = markup::native_component(node, diagnostics);

    let mut props_text = String::new();
    if let Some(style_name) = &base_style_ref {
        props_text.push_str(&format!(" style={{styles.{style_name}}}"));
    }
    for (key, value) in &extra_props {
        props_text.push_str(&format!(r#" {key}="{value}""#));
    }

    let inner = match &node.text {
        Some(TextContent::Literal(text)) => escape_jsx_text(text),
        Some(TextContent::Dynamic(_)) | None => node
            .children
            .iter()
            .map(|child| render_node(child, allocator, style_entries, diagnostics))
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
        let output = lower(root);

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
        assert!(output.styles.contains("paddingLeft: 16,"));
        // No `px`/CSS units anywhere -- these are unitless RN numbers.
        assert!(!output.styles.contains("px"));

        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn non_always_conditions_get_their_own_named_style_without_being_wired_into_style_prop() {
        let node = dowel_ir::Node {
            primitive: dowel_ir::Primitive::View,
            style: vec![
                dowel_ir::StyleDeclaration {
                    property: dowel_ir::StyleProperty::Opacity(1.0),
                    condition: dowel_ir::Condition::Always,
                },
                dowel_ir::StyleDeclaration {
                    property: dowel_ir::StyleProperty::Opacity(0.5),
                    condition: dowel_ir::Condition::Disabled,
                },
            ],
            props: dowel_ir::PropSet::default(),
            children: Vec::new(),
            text: None,
            class_name_fallback: Vec::new(),
            span: dowel_ir::SourceSpan { start: 0, end: 0 },
        };
        let output = lower(&node);
        assert!(output.jsx.contains("style={styles.dowel0}"));
        assert!(output.styles.contains("dowel0_disabled: {"));
        assert!(output.styles.contains("opacity: 0.5,"));
        // Known Phase 0 gap, asserted explicitly rather than left implicit:
        // the disabled-variant style exists but isn't merged into the
        // rendered style prop yet.
        assert!(!output.jsx.contains("dowel0_disabled"));
    }

    #[test]
    fn interactive_pressable_without_role_is_diagnosed_from_real_source() {
        // As with dowel_web: previously only reachable by hand-constructing
        // a `Node` -- the parser didn't populate on_press/accessibility_role
        // at all until dowel_parser::jsx gained that attribute parsing.
        let parsed = dowel_parser::parse_tsx(
            r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable onPress={handleTap}>Tap</Pressable>
            "#,
        );
        let output = lower(&parsed.roots[0]);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::A11yInteractiveWithoutRole);

        let parsed_with_role = dowel_parser::parse_tsx(
            r#"
            import { Pressable } from '@dowel/core'
            const el = (
              <Pressable onPress={handleTap} accessibilityRole="button">Tap</Pressable>
            )
            "#,
        );
        let output_with_role = lower(&parsed_with_role.roots[0]);
        assert!(output_with_role.diagnostics.is_empty());
        assert!(output_with_role.jsx.contains(r#"accessibilityRole="button""#));
    }
}
