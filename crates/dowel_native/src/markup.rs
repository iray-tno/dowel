//! `Node` -> React Native component name/props, plus the same
//! accessibility diagnostic `dowel_web::markup` emits (proposal
//! §10.1/§10.2) -- the diagnosis is platform-independent even though the
//! actual props differ (RN has no `role`/`tabIndex`, it has
//! `accessibilityRole`/`accessible`).

use dowel_ir::{AccessibilityRole, Diagnostic, DiagnosticCode, Node, Primitive, Severity};

/// `(RN component name, extra props beyond `style`)`.
///
/// `Button` maps to RN's `Pressable` with an explicit `accessibilityRole`
/// (proposal §10.1's own example), not RN's built-in `Button` component --
/// that component can't be styled the way a Dowel-compiled Button needs to
/// be (no `style` prop covering layout/typography, only a handful of color
/// props). `Pressable` gets the same interactive-without-role diagnostic as
/// the Web backend, using RN's actual accessibility prop names.
pub fn native_component(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> (&'static str, Vec<(&'static str, String)>) {
    match node.primitive {
        Primitive::View => ("View", Vec::new()),
        Primitive::Text => ("Text", Vec::new()),
        Primitive::Button => ("Pressable", vec![("accessibilityRole", "button".to_string())]),
        Primitive::Pressable => {
            let mut props = Vec::new();
            match node.props.accessibility_role {
                Some(AccessibilityRole::Button) => {
                    props.push(("accessibilityRole", "button".to_string()));
                }
                Some(AccessibilityRole::Link) => {
                    props.push(("accessibilityRole", "link".to_string()));
                }
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
            ("Pressable", props)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dowel_ir::{ExprRef, PropSet, SourceSpan};

    fn empty_span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }

    #[test]
    fn button_maps_to_pressable_with_explicit_role() {
        let node = Node {
            primitive: Primitive::Button,
            style: Vec::new(),
            props: PropSet::default(),
            children: Vec::new(),
            class_name_fallback: Vec::new(),
            span: empty_span(),
        };
        let mut diagnostics = Vec::new();
        let (component, props) = native_component(&node, &mut diagnostics);
        assert_eq!(component, "Pressable");
        assert!(props.iter().any(|(k, v)| *k == "accessibilityRole" && v == "button"));
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
        let (component, _) = native_component(&node, &mut diagnostics);
        assert_eq!(component, "Pressable");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::A11yInteractiveWithoutRole);
    }
}
