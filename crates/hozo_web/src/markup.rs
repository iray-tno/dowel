//! `Node` -> HTML element tag/attributes, plus the accessibility
//! diagnostics that fall out of that mapping (proposal §10.1/§10.2).

use hozo_ir::{AccessibilityRole, Diagnostic, DiagnosticCode, HeadingLevel, Node, Primitive, Severity};

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
    let (component, attrs) = element_shape_inner(node, diagnostics);
    (component, apply_authored_role(node, attrs, diagnostics))
}

fn element_shape_inner(node: &Node, diagnostics: &mut Vec<Diagnostic>) -> (&'static str, Vec<(&'static str, String)>) {
    match node.primitive {
        Primitive::View if node.props.on_layout.is_some() || node.props.has_responder_handlers() => ("View", Vec::new()),
        Primitive::View => ("div", Vec::new()),
        Primitive::Text if node.props.on_layout.is_some() => ("Text", Vec::new()),
        Primitive::Text => ("span", Vec::new()),
        Primitive::Paragraph if node.props.on_layout.is_some() => ("Paragraph", Vec::new()),
        Primitive::Paragraph => ("p", Vec::new()),
        Primitive::Heading if node.props.on_layout.is_some()
            || matches!(node.props.heading_level, Some(HeadingLevel::Dynamic(_))) => ("Heading", Vec::new()),
        Primitive::Heading => match node.props.heading_level {
            Some(HeadingLevel::Static(2)) => ("h2", Vec::new()),
            Some(HeadingLevel::Static(3)) => ("h3", Vec::new()),
            Some(HeadingLevel::Static(4)) => ("h4", Vec::new()),
            Some(HeadingLevel::Static(5)) => ("h5", Vec::new()),
            Some(HeadingLevel::Static(6)) => ("h6", Vec::new()),
            _ => ("h1", Vec::new()),
        },
        Primitive::Section if node.props.on_layout.is_some() => ("Section", Vec::new()),
        Primitive::Section => ("section", Vec::new()),
        Primitive::Article if node.props.on_layout.is_some() => ("Article", Vec::new()),
        Primitive::Article => ("article", Vec::new()),
        Primitive::Nav if node.props.on_layout.is_some() => ("Nav", Vec::new()),
        Primitive::Nav => ("nav", Vec::new()),
        Primitive::List if node.props.on_layout.is_some()
            || matches!(node.props.list_ordered, Some(hozo_ir::ConditionExpr::Ref(_))) => ("List", Vec::new()),
        Primitive::List if matches!(node.props.list_ordered, Some(hozo_ir::ConditionExpr::Static(true))) =>
            ("ol", Vec::new()),
        Primitive::List => ("ul", Vec::new()),
        Primitive::ListItem if node.props.on_layout.is_some() => ("ListItem", Vec::new()),
        Primitive::ListItem => ("li", Vec::new()),
        Primitive::Button => ("button", Vec::new()),
        Primitive::Link => ("a", Vec::new()),
        Primitive::Image if node.props.on_layout.is_some() || node.props.image_default_source.is_some() =>
            ("Image", image_attrs(node, diagnostics)),
        Primitive::Image => ("img", image_attrs(node, diagnostics)),
        Primitive::ScrollView if node.props.on_refresh.is_some()
            || node.props.refreshing.is_some()
            || node.props.on_layout.is_some()
            || node.props.on_scroll.is_some() =>
            ("ScrollView", Vec::new()),
        Primitive::ScrollView => ("div", Vec::new()),
        Primitive::FlatList => ("FlatList", Vec::new()),
        Primitive::Pressable => {
            let mut attrs = Vec::new();
            match &node.props.accessibility_role {
                Some(AccessibilityRole::Button) => attrs.push((if node.props.has_responder_handlers() { "accessibilityRole" } else { "role" }, "button".to_string())),
                Some(AccessibilityRole::Link) => attrs.push((if node.props.has_responder_handlers() { "accessibilityRole" } else { "role" }, "link".to_string())),
                // Any other ARIA role goes through as written. Hozo does
                // not have an opinion about `combobox` beyond carrying it
                // to the platform that understands it.
                Some(AccessibilityRole::Aria(role)) => attrs.push(("role", role.clone())),
                // A React Native container role. The DOM has nothing that
                // means it, and inventing the nearest ARIA role would be
                // announcing something the author did not write -- so it
                // is reported by `role_diagnostics` and no attribute is
                // emitted.
                Some(AccessibilityRole::NativeOnly(_)) => {}
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
            if node.props.on_press.is_some() && !node.props.has_responder_handlers() {
                // `tabIndex`, not `tabindex` -- this output is JSX, so DOM
                // props take React's camelCase spellings (same reason the
                // class attribute is emitted as `className`). React warns
                // on the all-lowercase form and drops it.
                attrs.push(("tabIndex", "0".to_string()));
            }
            (if node.props.has_responder_handlers() { "Pressable" } else { "div" }, attrs)
        }
        Primitive::TextInput => ("input", missing_label(node, diagnostics)),
        // Lowered to `@hozo/a11y`'s component, not to a bare `<dialog>`:
        // the element gives the trap and the inert background, but only
        // once something calls `showModal()`, and keeping `open` in step
        // with the DOM is exactly the runtime behaviour §10.3 assigns to a
        // runtime.
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

/// Diagnoses a text field with no accessible name (proposal §10.2).
///
/// Returns no attributes -- the label itself is passed through from source,
/// since Hozo never invents an accessible name. Guessing one from a
/// `placeholder` or a nearby heading is how a field ends up announced as
/// something it isn't, which is worse than being announced as nothing.
///
/// A `placeholder` in place of a label is called out specifically because
/// it is the common wrong answer: it is not reliably announced as a name,
/// and it disappears on the first keystroke -- exactly when someone would
/// want to check what the field was for.
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
                "TextInput has a placeholder but no accessible name. A placeholder is not a \
                 label: it may not be announced as one, and it disappears as soon as the user \
                 types. Add `accessibilityLabel`, or associate a visible <label>."
                    .to_string()
            } else {
                "TextInput has no accessible name, so a screen reader announces only that it is \
                 a text field. Add `accessibilityLabel`, or associate a visible <label>."
                    .to_string()
            },
            span: node.span,
        });
    }
    Vec::new()
}

pub fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use hozo_ir::{ExprRef, PropSet, SourceSpan};

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
                accessibility_role: Some(AccessibilityRole::Button),
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
        let (_, attrs) = element_shape(&node, &mut diagnostics);
        assert!(attrs.iter().any(|(k, v)| *k == "role" && v == "button"));
        assert!(diagnostics.is_empty());
    }
}

/// Applies an author-written role over the primitive's own.
///
/// `role` and `accessibilityRole` are two spellings of one concept, so a
/// primitive that supplies its own must not also emit it when the author
/// has said otherwise -- `<List role="menu">` is a menu built on a list,
/// and announcing both is announcing neither.
///
/// The author's role is never dropped for being redundant. `<ul role="list">`
/// looks redundant and is a deliberate, documented workaround: Safari
/// removes list semantics from a `<ul>` styled `list-style: none`, and the
/// explicit role is what puts them back.
fn apply_authored_role(
    node: &Node,
    mut attrs: Vec<(&'static str, String)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<(&'static str, String)> {
    let Some(role) = &node.props.accessibility_role else { return attrs };
    attrs.retain(|(key, _)| *key != "role" && *key != "accessibilityRole");
    // An element carrying responder handlers is rendered by a runtime
    // component that takes React Native's prop names, not by a DOM element
    // -- so the role has to keep the spelling that component reads.
    let key = if node.props.has_responder_handlers() { "accessibilityRole" } else { "role" };
    match role {
        AccessibilityRole::Button => attrs.push((key, "button".to_string())),
        AccessibilityRole::Link => attrs.push((key, "link".to_string())),
        AccessibilityRole::Aria(name) => attrs.push((key, name.clone())),
        // The DOM has nothing that means a React Native container role,
        // and the nearest ARIA one would be announcing something the
        // author did not write.
        AccessibilityRole::NativeOnly(name) => diagnostics.push(Diagnostic {
            code: DiagnosticCode::RoleHasNoWebEquivalent,
            severity: Severity::Warning,
            message: if hozo_parser::aria::is_abstract_role(name) {
                format!("`{name}` is one of ARIA's abstract roles, which describe the ontology rather than any element -- the specification says not to write one. Nothing is emitted here, so the element is announced as whatever its tag says.")
            } else {
                format!("`{name}` is not a role ARIA defines, so nothing is emitted here and the element is announced as whatever its tag says. React Native has its own role vocabulary with names like this one; the two overlap for most roles and not for this.")
            },
            span: node.span,
        }),
    }
    attrs
}
