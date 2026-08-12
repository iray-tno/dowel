//! Converts oxc's JSX AST into `dowel_ir::Node` trees.
//!
//! Phase 0 scope only: static and dynamic `className` (see
//! `crate::dynamic_class`), `View`/`Text`/`Pressable`/`Button` primitives,
//! plain text children. Event handlers and other props are not modeled yet
//! -- unmapped attributes are silently dropped for now rather than routed
//! to `PropSet::passthrough`.

use dowel_ir::{Condition, Node, Primitive, PropSet, SourceSpan, StyleDeclaration, TextContent};
use oxc_ast::ast::{
    JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild, JSXElement, JSXElementName,
};
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};
use oxc_syntax::module_record::ModuleRecord;

use crate::dynamic_class;
use crate::tailwind;

fn to_span(span: Span) -> SourceSpan {
    SourceSpan { start: span.start, end: span.end }
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
    for attr_item in &el.opening_element.attributes {
        let JSXAttributeItem::Attribute(attr) = attr_item else {
            continue; // spread attributes: not modeled in this pass
        };
        let JSXAttributeName::Identifier(attr_name) = &attr.name else {
            continue;
        };
        if attr_name.name.as_str() != "className" {
            continue;
        }
        match &attr.value {
            Some(JSXAttributeValue::StringLiteral(literal)) => {
                for token in literal.value.split_whitespace() {
                    for property in tailwind::expand_utility(token) {
                        style.push(StyleDeclaration { property, condition: Condition::Always });
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

    Some(Node {
        primitive,
        style,
        props: PropSet::default(),
        children,
        text,
        class_name_fallback,
        span: to_span(el.span()),
    })
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
