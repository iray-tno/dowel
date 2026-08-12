//! Dowel IR to DOM/CSS/ARIA lowering (Web backend).
//!
//! Emits per-node scoped CSS classes rather than deduplicating/reusing
//! atomic utility classes across call sites -- a constraint carried over
//! from the cascade-ordering design discussion, since it's what lets
//! simple source-order-within-condition flattening (see `css.rs`) stay
//! correct without needing RNW/StyleX-style explicit priority tables.

mod css;
mod markup;

use dowel_ir::{Diagnostic, Node, Primitive};

pub struct LowerOutput {
    pub jsx: String,
    pub css: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// The proposal §8.1 "dowel-view" shared base style: applied to every
/// `View`, emitted once as a shared rule rather than duplicated per node.
const VIEW_BASE_CSS: &str = ".dowel-view {\n  \
    display: flex;\n  \
    flex-direction: column;\n  \
    flex-shrink: 0;\n  \
    position: relative;\n  \
    min-width: 0;\n  \
    box-sizing: border-box;\n\
}\n\n";

struct ClassAllocator {
    next: u32,
}

impl ClassAllocator {
    fn alloc(&mut self) -> String {
        let name = format!("dowel-{}", self.next);
        self.next += 1;
        name
    }
}

pub fn lower(root: &Node) -> LowerOutput {
    let mut allocator = ClassAllocator { next: 0 };
    let mut rules = String::new();
    let mut diagnostics = Vec::new();
    let mut uses_view_base = false;

    let jsx = render_node(root, &mut allocator, &mut rules, &mut diagnostics, &mut uses_view_base);

    let mut css = String::new();
    if uses_view_base {
        css.push_str(VIEW_BASE_CSS);
    }
    css.push_str(&rules);

    LowerOutput { jsx, css, diagnostics }
}

fn render_node(
    node: &Node,
    allocator: &mut ClassAllocator,
    rules: &mut String,
    diagnostics: &mut Vec<Diagnostic>,
    uses_view_base: &mut bool,
) -> String {
    let class_name = allocator.alloc();

    for (condition, props) in css::group_by_condition(&node.style) {
        let props = css::dedupe_last_wins(props);
        if props.is_empty() {
            continue;
        }
        rules.push_str(&css::render_rule(&class_name, &condition, &props));
        rules.push_str("\n\n");
    }

    let (tag, extra_attrs) = markup::element_shape(node, diagnostics);

    let mut classes = class_name;
    if node.primitive == Primitive::View {
        *uses_view_base = true;
        classes = format!("dowel-view {classes}");
    }

    // `className`, not `class` -- Dowel's Web output is consumed as JSX
    // (the Vite plugin splices it back into React source), not raw HTML.
    let mut attrs = format!(r#" className="{classes}""#);
    for (key, value) in &extra_attrs {
        attrs.push_str(&format!(r#" {key}="{value}""#));
    }
    // Structural placeholders for any Condition::Expr guards this node's
    // own declarations depend on -- always "false" here since there's no
    // live runtime wiring yet (that's `@dowel/runtime`'s job, later). This
    // exists so the attribute name a CSS selector expects and the
    // attribute a real runtime would toggle are provably the same string.
    for expr_ref in collect_expr_refs(node) {
        attrs.push_str(&format!(r#" {}="false""#, css::expr_ref_attribute(expr_ref)));
    }

    let inner = match &node.text {
        Some(dowel_ir::TextContent::Literal(text)) => markup::html_escape(text),
        Some(dowel_ir::TextContent::Dynamic(_)) | None => node
            .children
            .iter()
            .map(|child| render_node(child, allocator, rules, diagnostics, uses_view_base))
            .collect(),
    };

    format!("<{tag}{attrs}>{inner}</{tag}>")
}

fn collect_expr_refs(node: &Node) -> Vec<dowel_ir::ExprRef> {
    let mut refs = Vec::new();
    for decl in &node.style {
        if let dowel_ir::Condition::Expr(expr) = &decl.condition {
            collect_from_expr(expr, &mut refs);
        }
    }
    refs.sort_by_key(|r: &dowel_ir::ExprRef| (r.0.start, r.0.end));
    refs.dedup();
    refs
}

fn collect_from_expr(expr: &dowel_ir::ConditionExpr, out: &mut Vec<dowel_ir::ExprRef>) {
    use dowel_ir::ConditionExpr;
    match expr {
        ConditionExpr::Ref(r) => out.push(*r),
        ConditionExpr::Not(inner) => collect_from_expr(inner, out),
        ConditionExpr::And(a, b) | ConditionExpr::Or(a, b) => {
            collect_from_expr(a, out);
            collect_from_expr(b, out);
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
    fn lowers_the_login_example_to_html_and_css() {
        let parsed = dowel_parser::parse_tsx(LOGIN_EXAMPLE);
        let root = &parsed.roots[0];
        let output = lower(root);

        assert!(output.jsx.starts_with(r#"<div className="dowel-view dowel-0">"#));
        assert!(output.jsx.contains("<span className=\"dowel-1\">Welcome</span>"));
        assert!(output.jsx.contains("<button className=\"dowel-2\">Continue</button>"));

        assert!(output.css.contains(".dowel-view {"));
        assert!(output.css.contains(".dowel-0 {"));
        assert!(output.css.contains("flex: 1 1 0%;"));
        assert!(output.css.contains("padding-top: 24px;"));
        assert!(output.css.contains(".dowel-1 {"));
        assert!(output.css.contains("font-size: 20px;"));
        assert!(output.css.contains("font-weight: 700;"));
        assert!(output.css.contains(".dowel-2 {"));
        assert!(output.css.contains("padding-left: 16px;"));

        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn hover_condition_compiles_to_a_real_pseudo_class() {
        // `hover:` isn't parsed from Tailwind source yet (dowel_parser's
        // table doesn't recognize variant prefixes), so this is exercised
        // directly at the IR level rather than through `parse_tsx`.
        let node = dowel_ir::Node {
            primitive: dowel_ir::Primitive::View,
            style: vec![dowel_ir::StyleDeclaration {
                property: dowel_ir::StyleProperty::Opacity(0.5),
                condition: dowel_ir::Condition::Hover,
            }],
            props: dowel_ir::PropSet::default(),
            children: Vec::new(),
            text: None,
            class_name_fallback: Vec::new(),
            span: dowel_ir::SourceSpan { start: 0, end: 0 },
        };
        let output = lower(&node);
        assert!(output.css.contains(".dowel-0:hover {"));
        assert!(output.css.contains("opacity: 0.5;"));
    }
}
