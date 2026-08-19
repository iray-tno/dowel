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
//
// The second and third need the tree, and the tree is only sometimes
// knowable: a `Child::Verbatim` between two elements may render nothing,
// one element, or a hundred. Where that happens this says nothing at all
// rather than guessing, which is the same rule the rest of the compiler
// follows.

use hozo_ir::{AccessibilityRole, Child, Diagnostic, DiagnosticCode, Node, Severity};

use crate::aria;

/// The ARIA state properties `accessibilityState` can supply.
///
/// It is one expression carrying an object, and Hozo does not read inside
/// it -- the Web backend emits `({expr}).expanded` and lets the value
/// decide. So a node with one may or may not supply any of these, and the
/// honest answer for all four is "cannot tell".
const STATE_PROPS: &[&str] = &["aria-disabled", "aria-selected", "aria-busy", "aria-expanded"];

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
        }
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
/// `None` means "cannot tell": an `accessibilityState` is one opaque
/// expression and its keys are not read, so every state it could carry has
/// to be treated as possibly supplied.
fn supplies(node: &Node, property: &str) -> Option<bool> {
    if node.props.accessibility_state.is_some() && STATE_PROPS.contains(&property) {
        return None;
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

fn list(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("`{item}`")).collect();
    match quoted.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}
