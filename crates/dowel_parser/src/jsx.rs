//! Converts oxc's JSX AST into `dowel_ir::Node` trees.
//!
//! Phase 0 scope: static and dynamic `className` (see
//! `crate::dynamic_class`), `View`/`Text`/`Pressable`/`Button` primitives,
//! plain text children, and `onPress`/`disabled`/`accessibilityRole`
//! props. Everything else is silently dropped for now rather than routed
//! to `PropSet::passthrough`.

use dowel_ir::{
    AccessibilityRole, ConditionExpr, ExprRef, Node, Primitive, PropSet, SourceSpan,
    StyleDeclaration, TextContent,
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
fn build_node(el: &JSXElement, module_record: &ModuleRecord) -> Option<Node> {
    let JSXElementName::IdentifierReference(ident) = &el.opening_element.name else {
        return None;
    };
    let primitive = primitive_for_name(ident.name.as_str())?;

    let mut style: Vec<StyleDeclaration> = Vec::new();
    let mut class_name_fallback = Vec::new();
    let mut props = PropSet::default();
    for attr_item in &el.opening_element.attributes {
        let JSXAttributeItem::Attribute(attr) = attr_item else {
            continue; // spread attributes: not modeled in this pass
        };
        let JSXAttributeName::Identifier(attr_name) = &attr.name else {
            continue;
        };
        match attr_name.name.as_str() {
            "className" => match &attr.value {
                Some(JSXAttributeValue::StringLiteral(literal)) => {
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
                }
                _ => {}
            },
            // Opaque, like a className-guard condition: never evaluated,
            // just threaded through by span for a later codegen stage to
            // re-emit verbatim. The shorthand boolean form (`<Button
            // disabled />`, no value) isn't handled yet -- there's no
            // expression to take a span from.
            "onPress" => {
                if let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value {
                    props.on_press = Some(to_expr_ref(container.expression.span()));
                }
            }
            "disabled" => {
                if let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value {
                    props.disabled = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
            }
            "accessibilityRole" => {
                props.accessibility_role = accessibility_role_from_value(&attr.value);
            }
            _ => {}
        }
    }

    let mut children: Vec<Node> = Vec::new();
    let mut text: Option<TextContent> = None;
    for child in &el.children {
        match child {
            JSXChild::Element(child_el) => {
                if let Some(child_node) = build_node(child_el, module_record) {
                    children.push(child_node);
                }
            }
            JSXChild::Text(t) => {
                let trimmed = t.value.trim();
                if !trimmed.is_empty() {
                    text = Some(TextContent::Literal(trimmed.to_string()));
                }
            }
            // Fragments, expression containers, and spreads aren't modeled
            // in this pass.
            _ => {}
        }
    }

    Some(Node { primitive, style, props, children, text, class_name_fallback, span: to_span(el.span()) })
}

/// Collects every top-level (i.e. not nested inside another already-visited
/// JSX element) `Node` tree found while walking a `Program`.
pub struct JsxCollector<'r, 'a> {
    pub roots: Vec<Node>,
    module_record: &'r ModuleRecord<'a>,
}

impl<'r, 'a> JsxCollector<'r, 'a> {
    pub fn new(module_record: &'r ModuleRecord<'a>) -> Self {
        Self { roots: Vec::new(), module_record }
    }
}

impl<'r, 'a> Visit<'a> for JsxCollector<'r, 'a> {
    fn visit_jsx_element(&mut self, it: &JSXElement<'a>) {
        // Deliberately does not call `walk_jsx_element` -- `build_node`
        // already recurses into children itself, so falling through to the
        // generic walker here would visit (and re-collect) nested elements
        // a second time.
        if let Some(node) = build_node(it, self.module_record) {
            self.roots.push(node);
        }
    }
}

#[cfg(test)]
mod tests {
    use dowel_ir::{AccessibilityRole, ConditionExpr};

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
