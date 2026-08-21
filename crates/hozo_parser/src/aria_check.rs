// Compile-time checks against the ARIA specification.
//
// Proposal §10.2, for the part of the surface Hozo's semantic primitives
// cannot reach. `<Section>` becomes `<section>` and carries its role for
// free; a combobox, a tab strip or a tree has no element to become, so the
// author writes ARIA by hand -- and hand-written ARIA is where the
// mistakes are. An incomplete pattern is not a crash and not a visual
// defect: it renders perfectly and is simply wrong to anyone using a
// screen reader, which is the least likely thing to be noticed in review.
//
// What is checked comes from `aria.rs`, generated from the specification
// itself, so the list is not a set of rules somebody thought of. Three
// kinds are derivable from a source file:
//
//   - a role's required states and properties
//   - the role it must be contained by
//   - the roles it must contain
//   - the states and properties it accepts, and the ones it refuses
//
// The second and third need the tree, and the tree is only sometimes
// knowable: a `Child::Verbatim` between two elements may render nothing,
// one element, or a hundred. Where that happens this says nothing at all
// rather than guessing, which is the same rule the rest of the compiler
// follows.

use hozo_ir::{AccessibilityRole, Child, Diagnostic, DiagnosticCode, Node, Primitive, Severity};

use crate::aria;

/// Whether `accessibilityState` is the prop that would carry `property`.
///
/// Read from `ARIA_STATE_KEYS` rather than restated. It was restated, and
/// the copy was missing `aria-checked` -- so a `role="checkbox"` written
/// with `accessibilityState={{ checked }}` was reported as missing the one
/// state it had.
fn is_state_prop(property: &str) -> bool {
    ARIA_STATE_KEYS.iter().any(|(aria_name, _)| *aria_name == property)
}

pub fn check(root: &Node, diagnostics: &mut Vec<Diagnostic>) {
    walk(root, &[], diagnostics);
}

fn walk(node: &Node, ancestors: &[&str], diagnostics: &mut Vec<Diagnostic>) {
    let own_role = aria_role(node);
    if let Some(name) = own_role {
        if let Some(spec) = aria::role(name) {
            check_props(node, spec, diagnostics);
            check_context(node, spec, ancestors, diagnostics);
            check_owned(node, spec, diagnostics);
            check_allowed(node, spec, diagnostics);
        }
    }
    // The name check also applies where no role was written, because the
    // role is then whatever the element is -- and for `View`, `Text` and
    // an unrolled `Pressable` that is `generic`, which prohibits a name.
    if let Some(spec) = own_role.or_else(|| implicit_role(node)).and_then(aria::role) {
        check_name_allowed(node, spec, diagnostics);
    }

    let mut inner: Vec<&str> = ancestors.to_vec();
    if let Some(name) = own_role {
        inner.push(name);
    }
    for child in &node.children {
        if let Child::Node(child_node) = child {
            walk(child_node, &inner, diagnostics);
        }
    }
}

/// The ARIA role a node carries, if it is one the specification names.
fn aria_role(node: &Node) -> Option<&str> {
    match &node.props.accessibility_role {
        Some(AccessibilityRole::Button) => Some("button"),
        Some(AccessibilityRole::Link) => Some("link"),
        Some(AccessibilityRole::Aria(name)) => Some(name.as_str()),
        Some(AccessibilityRole::NativeOnly(_)) | None => None,
    }
}

/// Whether the node supplies an ARIA property, under any of its spellings.
///
/// `None` means "cannot tell", which is this module's answer wherever the
/// source stops being readable.
fn supplies(node: &Node, property: &str) -> Option<bool> {
    if node.props.accessibility_state.is_some() && is_state_prop(property) {
        match node.props.accessibility_state_keys.as_ref() {
            // An object literal says exactly which states it carries, so
            // the ones it does not name are genuinely absent rather than
            // unknown -- and a required state that is missing from a
            // literal is now a finding instead of a shrug.
            Some(_) if written_props(node).contains(&property) => return Some(true),
            Some(_) => {}
            // A variable or a spread could carry anything.
            None => return None,
        }
    }
    let modelled = match property {
        "aria-label" => node.props.accessibility_label.is_some(),
        "aria-description" => node.props.accessibility_hint.is_some(),
        _ => false,
    };
    if modelled {
        return Some(true);
    }
    // A `{...spread}` may carry anything, so it is the same "cannot tell".
    if node.props.passthrough.iter().any(|prop| prop.is_spread) {
        return None;
    }
    Some(
        node.props
            .passthrough
            .iter()
            .filter_map(|prop| prop.name.as_deref())
            .any(|name| name == property),
    )
}

fn check_props(node: &Node, spec: &aria::AriaRole, diagnostics: &mut Vec<Diagnostic>) {
    let missing: Vec<&str> = spec
        .required_props
        .iter()
        .copied()
        .filter(|property| supplies(node, property) == Some(false))
        .collect();
    if missing.is_empty() {
        return;
    }
    diagnostics.push(Diagnostic {
        code: DiagnosticCode::AriaIncompletePattern,
        severity: Severity::Warning,
        message: format!(
            "`role=\"{}\"` needs {} to mean anything, and this element has {} none of them. \
             The element renders correctly either way; what changes is what a screen reader \
             announces.",
            spec.name,
            list(spec.required_props),
            if missing.len() == spec.required_props.len() { "" } else { "only some of " },
        ),
        span: node.span,
    });
}

fn check_context(
    node: &Node,
    spec: &aria::AriaRole,
    ancestors: &[&str],
    diagnostics: &mut Vec<Diagnostic>,
) {
    if spec.required_context.is_empty() {
        return;
    }
    if spec.required_context.iter().any(|role| ancestors.contains(role)) {
        return;
    }
    // An unreadable expression anywhere above could be supplying the
    // container, so a missing one is only a finding when the whole chain
    // was visible.
    diagnostics.push(Diagnostic {
        code: DiagnosticCode::AriaIncompletePattern,
        severity: Severity::Warning,
        message: format!(
            "`role=\"{}\"` has to be inside {}, and nothing above it here is. Assistive \
             technology reads the two together; on its own this announces as ordinary content.",
            spec.name,
            list(spec.required_context),
        ),
        span: node.span,
    });
}

fn check_owned(node: &Node, spec: &aria::AriaRole, diagnostics: &mut Vec<Diagnostic>) {
    if spec.required_owned.is_empty() {
        return;
    }
    // Anything the compiler only carries may render the missing role, so
    // its presence makes the answer unknowable rather than negative.
    if node.children.iter().any(|child| matches!(child, Child::Verbatim { .. })) {
        return;
    }
    let owned: Vec<&str> = node
        .children
        .iter()
        .filter_map(|child| match child {
            Child::Node(child_node) => aria_role(child_node),
            _ => None,
        })
        .collect();
    if spec.required_owned.iter().any(|role| owned.contains(role)) {
        return;
    }
    diagnostics.push(Diagnostic {
        code: DiagnosticCode::AriaIncompletePattern,
        severity: Severity::Warning,
        message: format!(
            "`role=\"{}\"` has to contain {}, and none of its children carry that role. An \
             empty one of these is announced as an empty {}.",
            spec.name,
            list(spec.required_owned),
            spec.name,
        ),
        span: node.span,
    });
}

/// The role a primitive means when the author wrote none.
///
/// Only the ones that become an element with no role of its own. Every
/// other primitive lowers to something that carries one -- `Section` to
/// `<section>`, `Heading` to `<h2>` -- and the specification's entry for
/// that role is what applies.
fn implicit_role(node: &Node) -> Option<&'static str> {
    match node.primitive {
        Primitive::View | Primitive::Text | Primitive::ScrollView => Some("generic"),
        Primitive::Pressable if node.props.accessibility_role.is_none() => Some("generic"),
        Primitive::Paragraph => Some("paragraph"),
        _ => None,
    }
}

/// States and properties written by name, and the ARIA spelling of each.
///
/// `accessibilityState`'s keys are readable when it is an object literal;
/// `accessibility_state_keys` is `None` when it is not, and then nothing
/// here is claimed.
fn written_props(node: &Node) -> Vec<&'static str> {
    let mut written = Vec::new();
    if let Some(keys) = node.props.accessibility_state_keys.as_ref() {
        for (aria_name, key) in ARIA_STATE_KEYS {
            if keys.iter().any(|written_key| written_key == key) {
                written.push(*aria_name);
            }
        }
    }
    written
}

/// `accessibilityState`'s keys and the ARIA property each becomes. The
/// same mapping the Web backend emits from; it is one to one.
const ARIA_STATE_KEYS: &[(&str, &str)] = &[
    ("aria-disabled", "disabled"),
    ("aria-selected", "selected"),
    ("aria-checked", "checked"),
    ("aria-busy", "busy"),
    ("aria-expanded", "expanded"),
];

/// A state the role does not accept.
///
/// Only for states written out by name. An opaque `accessibilityState`
/// could carry anything, and the rest of this module's rule is that an
/// unreadable expression means "cannot tell" rather than "no".
fn check_allowed(node: &Node, spec: &aria::AriaRole, diagnostics: &mut Vec<Diagnostic>) {
    let refused: Vec<&str> = written_props(node)
        .into_iter()
        .filter(|prop| !aria::allows_prop(spec, prop))
        .collect();
    if refused.is_empty() {
        return;
    }
    diagnostics.push(Diagnostic {
        code: DiagnosticCode::AriaPropNotAllowed,
        severity: Severity::Warning,
        message: format!(
            "`role=\"{}\"` does not take {}. Assistive technology has nothing to do with {} \
             here, so the state is carried into the output and never announced.",
            spec.name,
            list(&refused),
            if refused.len() == 1 { "it" } else { "them" },
        ),
        span: node.span,
    });
}

/// A name on a role that forbids one.
fn check_name_allowed(node: &Node, spec: &aria::AriaRole, diagnostics: &mut Vec<Diagnostic>) {
    if node.props.accessibility_label.is_none() {
        return;
    }
    if aria::allows_prop(spec, "aria-label") {
        return;
    }
    diagnostics.push(Diagnostic {
        code: DiagnosticCode::AriaNameProhibited,
        severity: Severity::Warning,
        message: format!(
            "`role=\"{}\"` cannot carry an accessible name, so this `accessibilityLabel` may \
             never be announced. Name something that can be named instead -- a `Section` \
             becomes a region, an `Article` and a `Nav` are landmarks -- or give this element \
             a role that takes one.",
            spec.name,
        ),
        span: node.span,
    });
}

fn list(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("`{item}`")).collect();
    match quoted.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use hozo_ir::DiagnosticCode;

    fn codes(element: &str) -> Vec<DiagnosticCode> {
        let source = format!(
            "import {{ Pressable, View, Text, Paragraph, Section }} from '@hozo/core'\n\
             const el = {element}\n"
        );
        crate::parse_tsx(&source).diagnostics.into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_state_the_role_does_not_take_is_reported() {
        // `role="button"` accepts `aria-expanded`, `aria-busy` and
        // `aria-disabled`. Not `aria-selected` -- and the state was going
        // into the output regardless, where nothing would read it.
        assert_eq!(
            codes(r#"<Pressable accessibilityRole="button" accessibilityState={{ selected: s }} onPress={go}>x</Pressable>"#),
            vec![DiagnosticCode::AriaPropNotAllowed],
        );
        assert!(
            codes(r#"<Pressable accessibilityRole="button" accessibilityState={{ expanded: o }} onPress={go}>x</Pressable>"#)
                .is_empty()
        );
    }

    #[test]
    fn an_opaque_state_is_not_guessed_at() {
        // The rule the rest of this module follows: where the source stops
        // being readable, say nothing rather than something.
        assert!(
            codes(r#"<Pressable accessibilityRole="button" accessibilityState={state} onPress={go}>x</Pressable>"#)
                .is_empty()
        );
    }

    #[test]
    fn a_name_on_something_that_cannot_be_named_is_reported() {
        // `generic` is what a bare `<div>` or `<span>` is, and ARIA
        // prohibits naming it -- so the label is written, rendered, and
        // may never be announced.
        for element in [
            r#"<View accessibilityLabel="Sidebar" className="p-4">x</View>"#,
            r#"<Text accessibilityLabel="Hint">x</Text>"#,
            r#"<Paragraph accessibilityLabel="Intro">x</Paragraph>"#,
            r#"<Pressable accessibilityLabel="Open">x</Pressable>"#,
        ] {
            assert_eq!(codes(element), vec![DiagnosticCode::AriaNameProhibited], "{element}");
        }
    }

    #[test]
    fn a_name_on_something_that_can_be_named_is_not() {
        // Which is the whole point of the semantic primitives: `Section`
        // becomes a region, and a region takes a name.
        assert!(codes(r#"<Section accessibilityLabel="Messages">x</Section>"#).is_empty());
        assert!(
            codes(r#"<Pressable accessibilityRole="button" accessibilityLabel="Save" onPress={go}>x</Pressable>"#)
                .is_empty()
        );
    }

    #[test]
    fn a_required_state_written_as_a_literal_key_counts_as_supplied() {
        // `aria-checked` was missing from the hand-copied list of which
        // properties `accessibilityState` can carry, so a checkbox written
        // with the one state it needs was reported as missing it.
        assert!(
            codes(r#"<Pressable accessibilityRole="checkbox" accessibilityState={{ checked: c }} onPress={go}>x</Pressable>"#)
                .is_empty()
        );
        assert_eq!(
            codes(r#"<Pressable accessibilityRole="checkbox" onPress={go}>x</Pressable>"#),
            vec![DiagnosticCode::AriaIncompletePattern],
        );
    }
}

#[cfg(test)]
mod focus_tests {
    use hozo_ir::DiagnosticCode;

    fn codes(element: &str) -> Vec<DiagnosticCode> {
        let source = format!(
            "import {{ Pressable }} from '@hozo/core'\nconst el = {element}\n"
        );
        crate::parse_tsx(&source).diagnostics.into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn focusable_on_a_disabled_element_is_reported() {
        // Someone reaching for the APG's "focusable disabled" pattern,
        // which Hozo does not offer because Android cannot produce it.
        // See docs/decisions/001.
        assert_eq!(
            codes(r#"<Pressable accessibilityRole="button" disabled={d} focusable onPress={go}>x</Pressable>"#),
            vec![DiagnosticCode::FocusableDisabledUnsupported],
        );
    }

    #[test]
    fn focusable_false_on_a_disabled_element_is_not() {
        // It agrees with what `disabled` already does, so there is nothing
        // to say. Only reachable because a boolean literal is read as the
        // constant it is rather than as an expression.
        assert!(
            codes(r#"<Pressable accessibilityRole="button" disabled={d} focusable={false} onPress={go}>x</Pressable>"#)
                .is_empty()
        );
    }

    #[test]
    fn focusable_on_its_own_is_not_reported() {
        assert!(
            codes(r#"<Pressable accessibilityRole="button" focusable onPress={go}>x</Pressable>"#)
                .is_empty()
        );
    }
}

#[cfg(test)]
mod variant_tests {
    use hozo_ir::DiagnosticCode;

    fn codes(class_name: &str) -> Vec<DiagnosticCode> {
        let source = format!(
            "import {{ View }} from '@hozo/core'\nconst el = <View className=\"{class_name}\">x</View>\n"
        );
        crate::parse_tsx(&source).diagnostics.into_iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_tailwind_variant_hozo_does_not_compile_is_named() {
        // These produced no CSS, reached the DOM as nothing, and said
        // nothing. `group-hover:` is not an exotic class.
        for class_name in [
            "group-hover:bg-blue-500",
            "peer-checked:bg-blue-500",
            "open:bg-blue-500",
            "has-[:focus]:bg-blue-500",
        ] {
            assert_eq!(
                codes(class_name),
                vec![DiagnosticCode::TailwindVariantNotSupported],
                "{class_name}",
            );
        }
    }

    #[test]
    fn implementing_a_variant_is_what_removes_its_diagnostic() {
        // `aria-expanded:` was in the list above until it was
        // implemented. That is the whole shape of this: the diagnostic
        // names a gap, and closing the gap closes the diagnostic, rather
        // than someone remembering to edit a list.
        assert!(codes("aria-expanded:bg-blue-500").is_empty());
        assert!(codes("aria-checked:bg-blue-500").is_empty());
    }

    #[test]
    fn a_class_that_was_never_tailwinds_is_not_mentioned() {
        // The whole value of asking Tailwind for its own variant list:
        // a project's own class is not a gap in Hozo.
        for class_name in ["my-card", "group", "peer", "p-4", "hover:bg-blue-500", "md:hover:p-4"] {
            assert!(codes(class_name).is_empty(), "{class_name}");
        }
    }

    #[test]
    fn one_problem_gets_one_report() {
        // `data-[state=open]:` has brackets *and* a variant Hozo lacks.
        // The variant is the accurate half; reporting both was noise.
        assert_eq!(
            codes("data-[state=open]:bg-blue-500"),
            vec![DiagnosticCode::TailwindVariantNotSupported],
        );
    }
}

#[cfg(test)]
mod aria_variant_tests {
    use hozo_ir::{Condition, StyleDeclaration};

    fn conditions(class_name: &str) -> Vec<Condition> {
        let source = format!(
            "import {{ View }} from '@hozo/core'\nconst el = <View className=\"{class_name}\">x</View>\n"
        );
        crate::parse_tsx(&source).roots[0]
            .node
            .style
            .iter()
            .map(|StyleDeclaration { condition, .. }| condition.clone())
            .collect()
    }

    #[test]
    fn tailwinds_aria_states_are_recognised() {
        // The nine Tailwind names, read from Tailwind rather than
        // remembered. Each one used to compile to nothing and say nothing.
        for state in crate::tailwind_variants::ARIA_VARIANT_STATES {
            let conditions = conditions(&format!("aria-{state}:p-4"));
            assert!(
                conditions.iter().any(|c| matches!(c, Condition::Aria(name) if name == state)),
                "aria-{state}: was not recognised",
            );
        }
    }

    #[test]
    fn a_state_tailwind_does_not_name_is_not_invented() {
        // `aria-sort` takes four words and has no boolean form, so
        // Tailwind spells it `aria-[sort=ascending]:`. Accepting
        // `aria-sort:` would be Hozo inventing a variant.
        assert!(conditions("aria-sort:p-4").is_empty());
        assert!(conditions("aria-nonsense:p-4").is_empty());
    }
}
