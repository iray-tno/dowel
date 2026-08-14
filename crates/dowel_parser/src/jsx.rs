//! Converts oxc's JSX AST into `dowel_ir::Node` trees.
//!
//! Phase 0 scope: static and dynamic `className` (see
//! `crate::dynamic_class`), `View`/`Text`/`Pressable`/`Button` primitives,
//! plain text children, and `onPress`/`disabled`/`accessibilityRole`
//! props. Everything else is silently dropped for now rather than routed
//! to `PropSet::passthrough`.

use dowel_ir::{
    AccessibilityRole, ConditionExpr, Diagnostic, DiagnosticCode, ExprRef, Node, PassthroughProp,
    Primitive, PropSet, Severity, SourceSpan, StyleDeclaration, TextContent,
};
use oxc_ast::ast::{
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXElementName,
    JSXExpression,
};
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};
use oxc_syntax::module_record::ModuleRecord;

use crate::dynamic_class;
use crate::tailwind;

fn to_span(span: Span) -> SourceSpan {
    SourceSpan { start: span.start, end: span.end }
}

fn to_expr_ref(span: Span) -> ExprRef {
    ExprRef(to_span(span))
}

/// Only a statically-known `"button"`/`"link"` string literal is
/// recognized -- `accessibilityRole` drives real ARIA/native role output
/// and diagnostic suppression, so (unlike `onPress`/`disabled`, which stay
/// fully opaque) Dowel needs to actually know its value, not just its
/// span. A dynamic or unrecognized value is treated the same as absent:
/// conservative (the interactive-without-role diagnostic can still fire)
/// rather than guessing.
fn accessibility_role_from_value(value: &Option<JSXAttributeValue>) -> Option<AccessibilityRole> {
    let literal = match value {
        Some(JSXAttributeValue::StringLiteral(lit)) => Some(lit.value.as_str()),
        Some(JSXAttributeValue::ExpressionContainer(container)) => match &container.expression {
            JSXExpression::StringLiteral(lit) => Some(lit.value.as_str()),
            _ => None,
        },
        _ => None,
    };
    match literal {
        Some("button") => Some(AccessibilityRole::Button),
        Some("link") => Some(AccessibilityRole::Link),
        _ => None,
    }
}

fn primitive_for_name(name: &str) -> Option<Primitive> {
    match name {
        "View" => Some(Primitive::View),
        "Text" => Some(Primitive::Text),
        "Pressable" => Some(Primitive::Pressable),
        "Button" => Some(Primitive::Button),
        _ => None,
    }
}

/// Builds a `Node` from a JSX element recognized as a Dowel primitive.
/// Returns `None` for elements Dowel doesn't model in Phase 0 (unknown
/// components, intrinsic HTML tags, namespaced/member-expression names).
fn build_node(
    el: &JSXElement,
    module_record: &ModuleRecord,
    diagnostics: &mut Vec<Diagnostic>,
    consumed: &mut Vec<SourceSpan>,
) -> Option<Node> {
    let JSXElementName::IdentifierReference(ident) = &el.opening_element.name else {
        return None;
    };
    let primitive = primitive_for_name(ident.name.as_str())?;

    let mut style: Vec<StyleDeclaration> = Vec::new();
    let mut class_name_fallback = Vec::new();
    let mut props = PropSet::default();
    let mut seen_class_name = false;
    for attr_item in &el.opening_element.attributes {
        let attr = match attr_item {
            JSXAttributeItem::Attribute(attr) => attr,
            JSXAttributeItem::SpreadAttribute(spread) => {
                if seen_class_name {
                    // JSX resolves duplicate props last-wins, so a spread
                    // *after* className can override Dowel's compiled
                    // classes at runtime with whatever the spread carries.
                    // The spread is still emitted (dropping it would be
                    // worse) -- this just refuses to let it happen silently.
                    diagnostics.push(Diagnostic {
                        code: DiagnosticCode::UnsafePropSpreadAfterStyle,
                        severity: Severity::Warning,
                        message: "Prop spread appears after className and may override Dowel's \
                                  compiled styles at runtime. Move the spread before className."
                            .to_string(),
                        span: to_span(spread.span()),
                    });
                }
                props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(spread.span()), is_spread: true });
                continue;
            }
        };
        let JSXAttributeName::Identifier(attr_name) = &attr.name else {
            // Namespaced names (`xlink:href`) aren't modeled, but must
            // still survive to output rather than being dropped.
            props
                .passthrough
                .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false });
            continue;
        };
        match attr_name.name.as_str() {
            "className" => {
                seen_class_name = true;
                match &attr.value {
                    Some(JSXAttributeValue::StringLiteral(literal)) => {
                        // Every token here is compiled unconditionally, so
                        // the whole literal is accounted for and the
                        // candidate scan skips it (see `crate::scan`).
                        consumed.push(to_span(literal.span()));
                        for token in literal.value.split_whitespace() {
                            let (condition, properties) = tailwind::expand_utility(token);
                            for property in properties {
                                style.push(StyleDeclaration { property, condition: condition.clone() });
                            }
                        }
                    }
                    Some(JSXAttributeValue::ExpressionContainer(container)) => {
                        let decomposed =
                            dynamic_class::decompose_class_name(&container.expression, module_record);
                        style.extend(decomposed.declarations);
                        class_name_fallback.extend(decomposed.fallback);
                        consumed.extend(decomposed.consumed);
                    }
                    _ => {}
                }
            }
            // Opaque, like a className-guard condition: never evaluated,
            // just threaded through by span for a later codegen stage to
            // re-emit verbatim. The shorthand boolean form (`<Button
            // disabled />`, no value) isn't handled yet -- there's no
            // expression to take a span from, so it falls through to
            // passthrough instead of being silently dropped.
            "onPress" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.on_press = Some(to_expr_ref(container.expression.span()));
                }
                _ => props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
            },
            "disabled" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.disabled = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
            },
            "accessibilityRole" => {
                props.accessibility_role = accessibility_role_from_value(&attr.value);
                if props.accessibility_role.is_none() {
                    // A dynamic/unrecognized role isn't modeled, but must
                    // still reach the output -- Dowel only declines to
                    // *reason* about it, not to emit it.
                    props
                        .passthrough
                        .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false });
                }
            }
            _ => props
                .passthrough
                .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
        }
    }

    let mut children: Vec<Node> = Vec::new();
    let mut text: Option<TextContent> = None;
    // Cleared by anything that renders but doesn't become a `Node`, so a
    // consumer can tell whether `children` is positionally faithful. See
    // `Node::children_complete`.
    let mut children_complete = true;
    for child in &el.children {
        match child {
            JSXChild::Element(child_el) => {
                match build_node(child_el, module_record, diagnostics, consumed) {
                    Some(child_node) => children.push(child_node),
                    // A component Dowel doesn't model still renders, and
                    // still occupies a position among its siblings.
                    None => children_complete = false,
                }
            }
            JSXChild::Text(t) => {
                let trimmed = t.value.trim();
                if !trimmed.is_empty() {
                    text = Some(TextContent::Literal(trimmed.to_string()));
                    // Text and elements are mutually exclusive downstream
                    // (`text` wins), and on Native the text gets its own
                    // inserted wrapper element -- either way `children` no
                    // longer describes what renders.
                    children_complete = false;
                }
            }
            // Fragments, expression containers, and spreads aren't modeled
            // in this pass. `{cond && <A/>}` and `{items.map(...)}` in
            // particular can contribute any number of siblings.
            _ => children_complete = false,
        }
    }

    Some(Node {
        primitive,
        style,
        props,
        children,
        text,
        class_name_fallback,
        children_complete,
        span: to_span(el.span()),
    })
}

/// Collects every top-level (i.e. not nested inside another already-visited
/// JSX element) `Node` tree found while walking a `Program`.
pub struct JsxCollector<'r, 'a> {
    pub roots: Vec<Node>,
    /// Source-level diagnostics (things true of the written JSX itself,
    /// independent of which backend it later lowers to) -- as opposed to
    /// the lowering-level ones each backend raises during `lower()`.
    pub diagnostics: Vec<Diagnostic>,
    /// See `dynamic_class::Decomposed::consumed`.
    pub consumed: Vec<SourceSpan>,
    module_record: &'r ModuleRecord<'a>,
}

impl<'r, 'a> JsxCollector<'r, 'a> {
    pub fn new(module_record: &'r ModuleRecord<'a>) -> Self {
        Self { roots: Vec::new(), diagnostics: Vec::new(), consumed: Vec::new(), module_record }
    }
}

impl<'r, 'a> Visit<'a> for JsxCollector<'r, 'a> {
    fn visit_jsx_element(&mut self, it: &JSXElement<'a>) {
        // Deliberately does not call `walk_jsx_element` -- `build_node`
        // already recurses into children itself, so falling through to the
        // generic walker here would visit (and re-collect) nested elements
        // a second time.
        if let Some(node) = build_node(it, self.module_record, &mut self.diagnostics, &mut self.consumed) {
            self.roots.push(node);
        }
    }
}

#[cfg(test)]
mod tests {
    use dowel_ir::{AccessibilityRole, ConditionExpr, DiagnosticCode};

    /// Slices `source` at a passthrough prop's span, so tests assert on the
    /// text that will actually be re-emitted rather than raw offsets.
    fn passthrough_texts<'a>(source: &'a str, node: &dowel_ir::Node) -> Vec<&'a str> {
        node.props
            .passthrough
            .iter()
            .map(|p| &source[p.span.0.start as usize..p.span.0.end as usize])
            .collect()
    }

    #[test]
    fn unmodeled_props_and_spreads_are_preserved_verbatim() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View {...rest} className="p-4" onLayout={onLayout} testID="row" />
            "#;
        let output = crate::parse_tsx(source);
        let root = &output.roots[0];
        assert_eq!(
            passthrough_texts(source, root),
            vec!["{...rest}", "onLayout={onLayout}", r#"testID="row""#]
        );
        assert!(root.props.passthrough[0].is_spread);
        assert!(!root.props.passthrough[1].is_spread);
    }

    #[test]
    fn spread_after_class_name_is_diagnosed() {
        let output = crate::parse_tsx(
            r#"
            import { View } from '@dowel/core'
            const el = <View className="p-4" {...rest} />
            "#,
        );
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, DiagnosticCode::UnsafePropSpreadAfterStyle);
        // Still emitted despite the warning -- dropping it would be worse.
        assert_eq!(output.roots[0].props.passthrough.len(), 1);
    }

    #[test]
    fn spread_before_class_name_is_not_diagnosed() {
        // Safe ordering: className comes last, so it wins under JSX's
        // last-wins duplicate resolution -- nothing to warn about.
        let output = crate::parse_tsx(
            r#"
            import { View } from '@dowel/core'
            const el = <View {...rest} className="p-4" />
            "#,
        );
        assert!(output.diagnostics.is_empty());
        assert_eq!(output.roots[0].props.passthrough.len(), 1);
    }

    #[test]
    fn boolean_shorthand_disabled_falls_through_to_passthrough() {
        // No expression to take a span from, so it can't become a
        // ConditionExpr -- but it must still reach the output.
        let source = r#"
            import { Button } from '@dowel/core'
            const el = <Button disabled>Save</Button>
            "#;
        let output = crate::parse_tsx(source);
        let root = &output.roots[0];
        assert!(root.props.disabled.is_none());
        assert_eq!(passthrough_texts(source, root), vec!["disabled"]);
    }

    #[test]
    fn dynamic_accessibility_role_reaches_output_even_though_it_is_not_modeled() {
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable accessibilityRole={computedRole}>Go</Pressable>
            "#;
        let output = crate::parse_tsx(source);
        let root = &output.roots[0];
        assert_eq!(root.props.accessibility_role, None);
        assert_eq!(passthrough_texts(source, root), vec!["accessibilityRole={computedRole}"]);
    }

    #[test]
    fn parses_on_press_and_accessibility_role() {
        let output = crate::parse_tsx(
            r#"
            import { Pressable } from '@dowel/core'
            const el = (
              <Pressable onPress={handlePress} accessibilityRole="button">
                Go
              </Pressable>
            )
            "#,
        );
        let root = &output.roots[0];
        assert!(root.props.on_press.is_some());
        assert_eq!(root.props.accessibility_role, Some(AccessibilityRole::Button));
    }

    #[test]
    fn parses_disabled_as_an_opaque_condition_expr() {
        let output = crate::parse_tsx(
            r#"
            import { Button } from '@dowel/core'
            const el = <Button disabled={isLoading}>Save</Button>
            "#,
        );
        let root = &output.roots[0];
        assert!(matches!(root.props.disabled, Some(ConditionExpr::Ref(_))));
    }

    #[test]
    fn accessibility_role_link_is_recognized() {
        let output = crate::parse_tsx(
            r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable accessibilityRole="link">Home</Pressable>
            "#,
        );
        assert_eq!(output.roots[0].props.accessibility_role, Some(AccessibilityRole::Link));
    }

    #[test]
    fn dynamic_accessibility_role_is_not_recognized() {
        // Conservative on purpose (see `accessibility_role_from_value`'s
        // doc comment): a role Dowel can't verify statically is treated as
        // absent, so the interactive-without-role diagnostic can still
        // fire rather than being silently suppressed by an unknown value.
        let output = crate::parse_tsx(
            r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable accessibilityRole={computedRole}>Go</Pressable>
            "#,
        );
        assert_eq!(output.roots[0].props.accessibility_role, None);
    }
}
