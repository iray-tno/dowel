//! `Node` -> React Native component name/props, plus the same
//! accessibility diagnostic `hozo_web::markup` emits (proposal
//! §10.1/§10.2) -- the diagnosis is platform-independent even though the
//! actual props differ (RN has no `role`/`tabIndex`, it has
//! `accessibilityRole`/`accessible`).

use hozo_ir::{AccessibilityRole, Diagnostic, DiagnosticCode, Node, Primitive, Severity};

/// `(RN component name, extra props beyond `style`)`.
///
/// `Button` maps to RN's `Pressable` with an explicit `accessibilityRole`
/// (proposal §10.1's own example), not RN's built-in `Button` component --
/// that component can't be styled the way a Hozo-compiled Button needs to
/// be (no `style` prop covering layout/typography, only a handful of color
/// props). `Pressable` gets the same interactive-without-role diagnostic as
/// the Web backend, using RN's actual accessibility prop names.
pub fn native_component(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> (&'static str, Vec<(&'static str, String)>) {
    match node.primitive {
        Primitive::View => ("View", Vec::new()),
        Primitive::Text => ("Text", Vec::new()),
        Primitive::Paragraph => ("Text", Vec::new()),
        Primitive::Heading => ("Text", vec![("accessibilityRole", "header".to_string())]),
        Primitive::Section => ("View", Vec::new()),
        Primitive::Article => ("View", vec![("role", "article".to_string())]),
        Primitive::Nav => ("View", vec![("role", "navigation".to_string())]),
        Primitive::List => ("View", vec![("accessibilityRole", "list".to_string())]),
        Primitive::ListItem => ("View", vec![("role", "listitem".to_string())]),
        Primitive::Button => ("Pressable", vec![("accessibilityRole", "button".to_string())]),
        Primitive::Link => ("HozoLink", Vec::new()),
        Primitive::Image => ("Image", image_attrs(node, diagnostics)),
        Primitive::ScrollView => ("ScrollView", Vec::new()),
        Primitive::FlatList => ("FlatList", vec![("accessibilityRole", "list".to_string())]),
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
        // Same diagnosis as the Web backend, which is the point: the
        // accessibility question doesn't change between platforms even
        // though the prop names do.
        Primitive::TextInput => ("TextInput", missing_label(node, diagnostics)),
        Primitive::Dialog => ("HozoDialog", dialog_attrs(node, diagnostics)),
    }
}

fn image_attrs(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> Vec<(&'static str, String)> {
    if node.props.accessibility_label.is_none() {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::A11yMissingAccessibleName,
            severity: Severity::Warning,
            message: "Image has no alternative text. Add `alt` (use an empty string for a decorative image) or `accessibilityLabel`."
                .to_string(),
            span: node.span,
        });
    }
    Vec::new()
}

/// Diagnoses a text field with no accessible name (proposal §10.2). The
/// Web counterpart carries the reasoning; this is the same check with
/// React Native's prop names in the message.
/// A dialog's own diagnostics (proposal §10.3): it needs a name, and it
/// needs a way out.
///
/// The dismissal check is the one part of §10.3's quality bar a compiler
/// can see -- focus trapping and restoration are behaviours, but "there is
/// no `onClose`" is a missing prop. Escape on Web and the hardware back
/// button on Android both arrive there, so without it the modal ignores
/// both and reads as a trap.
fn dialog_attrs(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> Vec<(&'static str, String)> {
    if node.props.accessibility_label.is_none() {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::A11yMissingAccessibleName,
            severity: Severity::Warning,
            message: "Dialog has no accessible name, so a screen reader announces only that a \n                      dialog opened. Add `accessibilityLabel`."
                .to_string(),
            span: node.span,
        });
    }
    if !node.props.has_on_close {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::A11yDialogWithoutDismiss,
            severity: Severity::Warning,
            message: "Dialog has no `onClose`, so Escape and the Android back button do nothing \n                      and the modal is a trap. Add `onClose`."
                .to_string(),
            span: node.span,
        });
    }
    Vec::new()
}

fn missing_label(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> Vec<(&'static str, String)> {
    if node.props.accessibility_label.is_none() {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::A11yMissingAccessibleName,
            severity: Severity::Warning,
            message: if node.props.has_placeholder {
                "TextInput has a placeholder but no accessible name. A placeholder is not a \n                 label: it may not be announced as one, and it disappears as soon as the user \n                 types. Add `accessibilityLabel`."
                    .to_string()
            } else {
                "TextInput has no accessible name, so a screen reader announces only that it is \n                 a text field. Add `accessibilityLabel`."
                    .to_string()
            },
            span: node.span,
        });
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hozo_ir::{ExprRef, PropSet, SourceSpan};

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
                test_id: None,
                native_id: None,
                pointer_events: None,
                accessibility_state: None,
                accessibility_value: None,
                accessibility_live_region: None,
                on_layout: None,
                heading_level: None,
                list_ordered: None,
                on_scroll: None,
                scroll_event_throttle: None,
                disabled: None,
                accessibility_role: None,
                accessibility_label: None,
                accessibility_hint: None,
                image_src: None,
                image_default_source: None,
                scroll_horizontal: None,
                refreshing: None,
                on_refresh: None,
                keyboard_should_persist_taps: None,
                shows_vertical_scroll_indicator: None,
                shows_horizontal_scroll_indicator: None,
                has_placeholder: false,
                open: None,
                has_on_close: false,
                passthrough: Vec::new(),
                ..PropSet::default()
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
