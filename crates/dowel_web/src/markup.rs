//! `Node` -> HTML element tag/attributes, plus the accessibility
//! diagnostics that fall out of that mapping (proposal §10.1/§10.2).

use dowel_ir::{AccessibilityRole, Diagnostic, DiagnosticCode, Node, Primitive, Severity};

/// `(tag, extra attributes beyond class)`.
///
/// `Button` maps straight to `<button>` -- real semantic HTML beats an
/// ARIA role emulation (proposal's "prefer platform semantics" principle).
/// `Pressable` has no such native equivalent, so it stays a `<div>`: with
/// an explicit `accessibility_role` override it gets the matching ARIA
/// role; with none *and* an `on_press` handler (i.e. it's presented as
/// interactive) it's exactly the case proposal §10.2's diagnostic example
/// warns about, so that diagnostic is emitted here rather than silently
/// shipping an inaccessible interactive `<div>`.
pub fn element_shape(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> (&'static str, Vec<(&'static str, String)>) {
    match node.primitive {
        Primitive::View => ("div", Vec::new()),
        Primitive::Text => ("span", Vec::new()),
        Primitive::Button => ("button", Vec::new()),
        Primitive::Pressable => {
            let mut attrs = Vec::new();
            match node.props.accessibility_role {
                Some(AccessibilityRole::Button) => attrs.push(("role", "button".to_string())),
                Some(AccessibilityRole::Link) => attrs.push(("role", "link".to_string())),
                None => {
                    if node.props.on_press.is_some() {
                        diagnostics.push(Diagnostic {
                            code: DiagnosticCode::A11yInteractiveWithoutRole,
                            severity: Severity::Warning,
                            message: "Interactive Pressable has no accessible role. Consider: \
                                      accessibilityRole=\"button\""
                                .to_string(),
                            span: node.span,
                        });
                    }
                }
            }
            if node.props.on_press.is_some() {
                // `tabIndex`, not `tabindex` -- this output is JSX, so DOM
                // props take React's camelCase spellings (same reason the
                // class attribute is emitted as `className`). React warns
                // on the all-lowercase form and drops it.
                attrs.push(("tabIndex", "0".to_string()));
            }
            ("div", attrs)
        }
    }
}

pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use dowel_ir::{ExprRef, PropSet, SourceSpan};

    fn empty_span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }

    #[test]
    fn button_maps_to_native_button_element() {
        let node = Node {
            primitive: Primitive::Button,
            style: Vec::new(),
            props: PropSet::default(),
            children: Vec::new(),
            class_name_fallback: Vec::new(),
            span: empty_span(),
        };
        let mut diagnostics = Vec::new();
        let (tag, attrs) = element_shape(&node, &mut diagnostics);
        assert_eq!(tag, "button");
        assert!(attrs.is_empty());
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn interactive_pressable_without_role_gets_diagnosed() {
        let node = Node {
            primitive: Primitive::Pressable,
            style: Vec::new(),
            props: PropSet {
                on_press: Some(ExprRef(empty_span())),
                disabled: None,
                accessibility_role: None,
                passthrough: Vec::new(),
            },
            children: Vec::new(),
            class_name_fallback: Vec::new(),
            span: empty_span(),
        };
        let mut diagnostics = Vec::new();
        let (tag, attrs) = element_shape(&node, &mut diagnostics);
        assert_eq!(tag, "div");
        assert!(attrs.iter().any(|(k, v)| *k == "tabIndex" && v == "0"));
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::A11yInteractiveWithoutRole);
    }

    #[test]
    fn pressable_with_explicit_role_is_not_diagnosed() {
        let node = Node {
            primitive: Primitive::Pressable,
            style: Vec::new(),
            props: PropSet {
                on_press: Some(ExprRef(empty_span())),
                disabled: None,
                accessibility_role: Some(AccessibilityRole::Button),
                passthrough: Vec::new(),
            },
            children: Vec::new(),
            class_name_fallback: Vec::new(),
            span: empty_span(),
        };
        let mut diagnostics = Vec::new();
        let (_, attrs) = element_shape(&node, &mut diagnostics);
        assert!(attrs.iter().any(|(k, v)| *k == "role" && v == "button"));
        assert!(diagnostics.is_empty());
    }
}
