//! Converts oxc's JSX AST into `hozo_ir::Node` trees.
//!
//! Phase 0 scope: static and dynamic `className` (see
//! `crate::dynamic_class`), `View`/`Text`/`Pressable`/`Button` primitives,
//! text and element children, and `onPress`/`disabled`/`accessibilityRole`
//! props.
//!
//! Everything outside that scope is *carried*, not dropped: unmodeled
//! attributes go to `PropSet::passthrough`, and unmodeled children become
//! `Child::Verbatim` to be re-emitted from source. Not understanding
//! something is a reason to leave it alone, not a reason to delete it.

use hozo_ir::{
    AccessibilityRole, Child, ConditionExpr, Diagnostic, DiagnosticCode, ExprRef, HeadingLevel,
    NestedNode, Node, PassthroughProp, Primitive, PropSet, Severity, SourceSpan, StyleDeclaration,
};
use oxc_ast::ast::{
    ArrowFunctionExpression, Function, JSXAttribute, JSXAttributeItem, JSXAttributeName, JSXAttributeValue,
    JSXChild, JSXElement, JSXElementName, JSXExpression, ObjectPropertyKind, PropertyKey,
};
use oxc_ast_visit::walk::{walk_arrow_function_expression, walk_function, walk_jsx_element};
use oxc_ast_visit::Visit;
use oxc_syntax::scope::ScopeFlags;
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
/// fully opaque) Hozo needs to actually know its value, not just its
/// span. A dynamic or unrecognized value is treated the same as absent:
/// conservative (the interactive-without-role diagnostic can still fire)
/// rather than guessing.
fn accessibility_role_from_value(
    value: &Option<JSXAttributeValue>,
    aria: bool,
) -> Option<AccessibilityRole> {
    let literal = match value {
        Some(JSXAttributeValue::StringLiteral(lit)) => Some(lit.value.as_str()),
        Some(JSXAttributeValue::ExpressionContainer(container)) => match &container.expression {
            JSXExpression::StringLiteral(lit) => Some(lit.value.as_str()),
            _ => None,
        },
        _ => None,
    };
    role_from_literal(literal?, aria)
}

/// The ARIA roles React Native spells differently in its own vocabulary.
///
/// Only the renames. Every other React Native role name that means
/// something in ARIA *is* the ARIA name -- `checkbox`, `combobox`, `menu`,
/// `tablist` and the rest -- so membership is asked of the ARIA role list
/// rather than restated here, and a name absent from both is one this
/// platform invented.
const RN_ROLE_RENAMES: &[(&str, &str)] = &[
    ("header", "heading"),
    ("search", "searchbox"),
    ("image", "img"),
    ("adjustable", "slider"),
];

/// Reads a role literal, in either vocabulary.
///
/// `aria` says whether the attribute was spelled `role` -- in which case
/// the value is ARIA's by definition and no rename applies. React Native's
/// `accessibilityRole` gets the renames and then the same membership test.
fn role_from_literal(literal: &str, aria: bool) -> Option<AccessibilityRole> {
    let name = if aria {
        literal
    } else {
        RN_ROLE_RENAMES
            .iter()
            .find(|(rn, _)| *rn == literal)
            .map_or(literal, |(_, aria_name)| *aria_name)
    };
    Some(match name {
        // The two that decide an element rather than an attribute.
        "button" => AccessibilityRole::Button,
        "link" => AccessibilityRole::Link,
        _ if crate::aria::is_role(name) => AccessibilityRole::Aria(name.to_string()),
        // Not a role either vocabulary knows. Left to the backends: React
        // Native may still understand it, and the DOM will not.
        _ => AccessibilityRole::NativeOnly(literal.to_string()),
    })
}

/// What the walk needs to know about the module it is reading.
///
/// `module_record` was threaded through every builder already;
/// `foreign` rides with it because it answers the same kind of question --
/// which bindings this file actually has and where they came from.
pub(crate) struct Scope<'r, 'a> {
    pub module_record: &'r ModuleRecord<'a>,
    /// Primitive-named locals imported from a module the project does not
    /// trust.
    ///
    /// A tag naming one of these is carried verbatim rather than lowered,
    /// which is the whole of `@expo/ui` support: that package exports
    /// `Text`, `Button`, `List`, `ListItem`, `ScrollView` and
    /// `TextInput`, and every one of them is a native platform component
    /// with nothing in common with the Hozo primitive of the same name
    /// beyond the spelling.
    pub foreign: std::collections::HashSet<String>,
}

/// Finds and lowers Hozo primitives nested inside something the compiler
/// is only carrying, not reading -- an expression container, or an
/// unmodeled component's children.
///
/// The compiler can read these perfectly well; what it can't read is the
/// expression *around* them. So `show &&` is carried untouched while the
/// `<Text>` beside it compiles exactly as a top-level one would.
///
/// Lowers on the spot rather than collecting references to come back to:
/// the borrow the walk hands out doesn't outlive it, and threading the
/// build through the visitor is simpler than any way of extending it.
struct PrimitiveFinder<'r, 'a, 'd> {
    scope: &'r Scope<'r, 'a>,
    diagnostics: &'d mut Vec<Diagnostic>,
    consumed: &'d mut Vec<SourceSpan>,
    nested: Vec<NestedNode>,
}

impl<'r, 'a, 'd> Visit<'a> for PrimitiveFinder<'r, 'a, 'd> {
    fn visit_jsx_element(&mut self, it: &JSXElement<'a>) {
        if let JSXElementName::IdentifierReference(ident) = &it.opening_element.name {
            if self.scope.foreign.contains(ident.name.as_str()) {
                // Carried, not lowered -- and its own children are visited
                // by the walk that continues below, so a Hozo primitive
                // inside a foreign component still compiles.
            } else if let Some(name) = primitive_name(ident.name.as_str()) {
                match build_node(it, self.scope, self.diagnostics, self.consumed) {
                    Some(node) => {
                        self.nested.push(NestedNode { span: to_span(it.span()), node })
                    }
                    // Unreachable through this finder, which only matches
                    // the four identifier names `build_node` accepts --
                    // kept so a future widening of one and not the other
                    // degrades to a named gap rather than a silently
                    // uncompiled element.
                    None => self.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::PrimitiveNotLowered,
                        severity: Severity::Warning,
                        message: format!(
                            "This `<{name}>` is inside an expression the compiler doesn't read \
                             and couldn't be compiled in place. It falls back to the runtime \
                             component and gets its CSS from the project-wide candidate \
                             stylesheet instead of a scoped class."
                        ),
                        span: to_span(it.span()),
                    }),
                }
                // The outermost primitive on this branch. `build_node`
                // recurses into its children itself, so descending further
                // would compile them a second time.
                return;
            }
        }
        // Keeps descending otherwise: `<Avatar><Text/></Avatar>` and
        // `{rows.map(() => <Text/>)}` both hide one further down.
        oxc_ast_visit::walk::walk_jsx_element(self, it);
    }
}

/// Applies JSX's whitespace rules to a text child, matching what Babel and
/// TypeScript do so Hozo's output says what the source said.
///
/// The rules are not "trim". Whitespace *containing a newline* at either
/// end is dropped, which is what makes indented markup work; whitespace
/// within a line is significant, which is what makes `Hello {name}` keep
/// its space. Trimming instead -- as this did until 2026-08-15 -- silently
/// glued that pair together.
fn clean_jsx_text(raw: &str) -> String {
    let lines: Vec<&str> = raw.split(['\r', '\n']).collect();
    let last_non_empty = lines
        .iter()
        .rposition(|line| line.contains(|c: char| c != ' ' && c != '\t'))
        .unwrap_or(0);

    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        let mut trimmed = line.replace('\t', " ");
        if index != 0 {
            trimmed = trimmed.trim_start_matches(' ').to_string();
        }
        if index != lines.len() - 1 {
            trimmed = trimmed.trim_end_matches(' ').to_string();
        }
        if trimmed.is_empty() {
            continue;
        }
        out.push_str(&trimmed);
        // Lines that ran together in the source are joined by one space,
        // except after the last one that had content.
        if index != last_non_empty {
            out.push(' ');
        }
    }
    out
}

/// Whether a name is one the compiler lowers when it appears as a tag.
pub fn is_primitive_name(name: &str) -> bool {
    primitive_name(name).is_some()
}

fn primitive_name(name: &str) -> Option<&'static str> {
    match name {
        "View" => Some("View"),
        "Text" => Some("Text"),
        "Paragraph" => Some("Paragraph"),
        "Heading" => Some("Heading"),
        "Section" => Some("Section"),
        "Article" => Some("Article"),
        "Nav" => Some("Nav"),
        "List" => Some("List"),
        "ListItem" => Some("ListItem"),
        "Pressable" => Some("Pressable"),
        "Button" => Some("Button"),
        "Link" => Some("Link"),
        "TextInput" => Some("TextInput"),
        "Dialog" => Some("Dialog"),
        "Image" => Some("Image"),
        "ScrollView" => Some("ScrollView"),
        "FlatList" => Some("FlatList"),
        _ => None,
    }
}

/// Builds a `Child::Verbatim` for a child the compiler carries rather than
/// reads, lowering any Hozo primitives nested inside it.
///
/// The expression is opaque; the primitives in it are not. So `show &&` is
/// left alone while the `<Text>` beside it compiles normally.
fn carry_verbatim(
    child: &JSXChild,
    span: Span,
    scope: &Scope,
    diagnostics: &mut Vec<Diagnostic>,
    consumed: &mut Vec<SourceSpan>,
) -> Child {
    let mut finder = PrimitiveFinder {
        scope,
        diagnostics,
        consumed,
        nested: Vec::new(),
    };
    finder.visit_jsx_child(child);
    Child::Verbatim { source: to_expr_ref(span), nested: finder.nested }
}

fn passthrough_prop(
    attr: &JSXAttribute,
    scope: &Scope,
    diagnostics: &mut Vec<Diagnostic>,
    consumed: &mut Vec<SourceSpan>,
) -> PassthroughProp {
    let mut finder = PrimitiveFinder {
        scope,
        diagnostics,
        consumed,
        nested: Vec::new(),
    };
    finder.visit_jsx_attribute(attr);
    PassthroughProp {
        span: to_expr_ref(attr.span()),
        is_spread: false,
        // A namespaced name (`xlink:href`) is not something Hozo emits a
        // semantic counterpart for, so leaving it `None` costs nothing and
        // avoids inventing a spelling for it.
        name: match &attr.name {
            JSXAttributeName::Identifier(name) => Some(name.name.to_string()),
            JSXAttributeName::NamespacedName(_) => None,
        },
        nested: finder.nested,
    }
}

fn capture_prop_expr(
    attr: &JSXAttribute,
    target: &mut Option<ExprRef>,
    passthrough: &mut Vec<PassthroughProp>,
    scope: &Scope,
    diagnostics: &mut Vec<Diagnostic>,
    consumed: &mut Vec<SourceSpan>,
) {
    *target = match &attr.value {
        Some(JSXAttributeValue::ExpressionContainer(container)) => {
            Some(to_expr_ref(container.expression.span()))
        }
        Some(JSXAttributeValue::StringLiteral(literal)) => Some(to_expr_ref(literal.span)),
        _ => {
            passthrough.push(passthrough_prop(attr, scope, diagnostics, consumed));
            None
        }
    };
}

/// The property names an object-literal attribute writes, if all of them
/// can be read from the source.
///
/// Returns `None` for anything that leaves the set open -- a spread, a
/// computed key, or an expression that is not an object literal at all.
/// Guessing there would be worse than not knowing: the caller uses this to
/// decide which attributes are safe to read off the value.
fn object_literal_keys(attr: &JSXAttribute) -> Option<Vec<String>> {
    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
        return None;
    };
    let JSXExpression::ObjectExpression(object) = &container.expression else {
        return None;
    };
    let mut keys = Vec::new();
    for property in &object.properties {
        let ObjectPropertyKind::ObjectProperty(property) = property else {
            return None;
        };
        if property.computed {
            return None;
        }
        match &property.key {
            PropertyKey::StaticIdentifier(name) => keys.push(name.name.to_string()),
            PropertyKey::StringLiteral(literal) => keys.push(literal.value.to_string()),
            _ => return None,
        }
    }
    Some(keys)
}

/// The Tailwind variant in `token` that Hozo does not compile, if that is
/// why nothing was produced for it.
///
/// `None` for a class that was never Tailwind's. A project's own `my-card`
/// is not a gap in Hozo and saying so would be noise -- the whole value of
/// this is telling the two apart, which needs Tailwind's own list rather
/// than a set of prefixes somebody remembered.
///
/// Reads the prefix Hozo could not strip: `parse_variant_prefix` removes
/// the variants it implements, so whatever colon is left is the first one
/// it did not.
fn unsupported_variant(token: &str) -> Option<&str> {
    let (_, rest) = tailwind::parse_variant_prefix(token);
    let (prefix, _) = rest.split_once(':')?;
    // An arbitrary variant (`[&:hover]:p-4`) is a different report -- it
    // is not a name Tailwind defines, and `is_arbitrary` covers it.
    if prefix.starts_with('[') {
        return None;
    }
    crate::tailwind_variants::is_variant(prefix).then_some(prefix)
}

/// `focusable` on an element that is also `disabled`.
///
/// This is someone reaching for the ARIA APG's "focusable disabled"
/// pattern, which Hozo does not offer -- Android cannot produce it. React
/// Native routes `disabled` to `View.setEnabled(false)`, and a view that
/// is not enabled cannot take input focus however `focusable` is set, so
/// the prop would work on Web and quietly do nothing there.
///
/// Reported rather than ignored, and the message says what to do instead.
/// `focusable={false}` is not reported: it agrees with what `disabled`
/// already does, so there is nothing to warn about.
fn validate_focusable_disabled(
    props: &PropSet,
    span: SourceSpan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(focusable) = props.focusable.as_ref() else { return };
    if props.disabled.is_none() {
        return;
    }
    // `focusable={false}` agrees with what `disabled` already does, so
    // there is nothing to say about it. Anything else is someone asking
    // for a state Hozo does not offer.
    if matches!(focusable, ConditionExpr::Static(false)) {
        return;
    }
    diagnostics.push(Diagnostic {
        code: DiagnosticCode::FocusableDisabledUnsupported,
        severity: Severity::Warning,
        message: "`focusable` has no effect on a disabled element. React Native's disabled \
                  state removes keyboard focus on Android, and Hozo cannot yet separate that \
                  from the announcement. To let people reach the control and learn why it is \
                  unavailable, leave it enabled and answer in the handler instead."
            .to_string(),
        span,
    });
}

fn primitive_for_name(name: &str) -> Option<Primitive> {
    match name {
        "View" => Some(Primitive::View),
        "Text" => Some(Primitive::Text),
        "Paragraph" => Some(Primitive::Paragraph),
        "Heading" => Some(Primitive::Heading),
        "Section" => Some(Primitive::Section),
        "Article" => Some(Primitive::Article),
        "Nav" => Some(Primitive::Nav),
        "List" => Some(Primitive::List),
        "ListItem" => Some(Primitive::ListItem),
        "Pressable" => Some(Primitive::Pressable),
        "TextInput" => Some(Primitive::TextInput),
        "Dialog" => Some(Primitive::Dialog),
        "Button" => Some(Primitive::Button),
        "Link" => Some(Primitive::Link),
        "Image" => Some(Primitive::Image),
        "ScrollView" => Some(Primitive::ScrollView),
        "FlatList" => Some(Primitive::FlatList),
        _ => None,
    }
}

/// Builds a `Node` from a JSX element recognized as a Hozo primitive.
/// Returns `None` for elements Hozo doesn't model in Phase 0 (unknown
/// components, intrinsic HTML tags, namespaced/member-expression names).
fn build_node(
    el: &JSXElement,
    scope: &Scope,
    diagnostics: &mut Vec<Diagnostic>,
    consumed: &mut Vec<SourceSpan>,
) -> Option<Node> {
    let JSXElementName::IdentifierReference(ident) = &el.opening_element.name else {
        return None;
    };
    // A name the project's own components own. Declining here is what
    // turns it into a `Child::Verbatim` at the call site -- carried and
    // re-emitted from source, the same treatment any unmodeled component
    // gets -- so the tree around it still compiles.
    if scope.foreign.contains(ident.name.as_str()) {
        return None;
    }
    let primitive = primitive_for_name(ident.name.as_str())?;

    let mut style: Vec<StyleDeclaration> = Vec::new();
    let mut class_name_fallback = Vec::new();
    let mut carried_classes: Vec<String> = Vec::new();
    let mut props = PropSet::default();
    let mut seen_class_name = false;
    for attr_item in &el.opening_element.attributes {
        let attr = match attr_item {
            JSXAttributeItem::Attribute(attr) => attr,
            JSXAttributeItem::SpreadAttribute(spread) => {
                if seen_class_name {
                    // JSX resolves duplicate props last-wins, so a spread
                    // *after* className can override Hozo's compiled
                    // classes at runtime with whatever the spread carries.
                    // The spread is still emitted (dropping it would be
                    // worse) -- this just refuses to let it happen silently.
                    diagnostics.push(Diagnostic {
                        code: DiagnosticCode::UnsafePropSpreadAfterStyle,
                        severity: Severity::Warning,
                        message: "Prop spread appears after className and may override Hozo's \
                                  compiled styles at runtime. Move the spread before className."
                            .to_string(),
                        span: to_span(spread.span()),
                    });
                }
                props
                    .passthrough
                    .push(PassthroughProp {
                        span: to_expr_ref(spread.span()),
                        is_spread: true,
                        name: None,
                        nested: Vec::new(),
                    });
                continue;
            }
        };
        let JSXAttributeName::Identifier(attr_name) = &attr.name else {
            // Namespaced names (`xlink:href`) aren't modeled, but must
            // still survive to output rather than being dropped.
            props
                .passthrough
                .push(passthrough_prop(attr, scope, diagnostics, consumed));
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
                            // Several groups only for a shorthand like
                            // `container`, which is a width plus a
                            // max-width at each breakpoint.
                            let groups = tailwind::expand_class(token);
                            let properties: Vec<_> =
                                groups.iter().flat_map(|(_, p)| p.clone()).collect();
                            // Nothing recognised, so it goes back into the
                            // element rather than being deleted from it.
                            // "Hozo leaves it alone" is what the comment
                            // below has always said and was not true: an
                            // unknown class was dropped, taking a
                            // project's own `my-card` with it, and
                            // Tailwind's `group` and `peer` -- marker
                            // classes with no styles of their own, whose
                            // whole purpose is to be selected against by a
                            // descendant.
                            let unsupported = properties
                                .is_empty()
                                .then(|| unsupported_variant(token))
                                .flatten();
                            if properties.is_empty() {
                                carried_classes.push(token.to_string());
                            }
                            if let Some(variant) = unsupported {
                                diagnostics.push(Diagnostic {
                                    code: DiagnosticCode::TailwindVariantNotSupported,
                                    severity: Severity::Warning,
                                    message: format!(
                                        "`{variant}:` is a Tailwind variant Hozo does not \
                                         compile yet, so `{token}` produces no style. The class \
                                         is still on the element, so it will work if the \
                                         project runs its own Tailwind build over it."
                                    ),
                                    span: to_span(literal.span()),
                                });
                            }
                            // Reported only for brackets. An unknown bare
                            // class is ordinary -- projects have their own
                            // CSS -- but a bracket is unambiguously
                            // Tailwind being asked for something, so
                            // failing to read one is worth saying out
                            // loud. It stayed silent until 2026-08-16,
                            // which is how `w-[32px]` came to compile to
                            // nothing at all.
                            // Skipped when the variant was already named:
                            // one problem, one report. `data-[state=open]:`
                            // has brackets *and* a variant Hozo lacks, and
                            // the variant is the accurate half.
                            if unsupported.is_none() && properties.is_empty() && tailwind::is_arbitrary(token) {
                                diagnostics.push(Diagnostic {
                                    code: DiagnosticCode::UnreadableArbitraryValue,
                                    severity: Severity::Warning,
                                    message: format!(
                                        "`{token}` uses Tailwind's arbitrary syntax and Hozo \
                                         couldn't read it, so no style is generated for it. The \
                                         class still reaches the DOM, so a hand-written rule for \
                                         it will still apply."
                                    ),
                                    span: to_span(literal.span()),
                                });
                            }
                            for (condition, properties) in groups {
                                for property in properties {
                                    style.push(StyleDeclaration {
                                        property,
                                        condition: condition.clone(),
                                    });
                                }
                            }
                        }
                    }
                    Some(JSXAttributeValue::ExpressionContainer(container)) => {
                        let decomposed =
                            dynamic_class::decompose_class_name(&container.expression, scope.module_record);
                        style.extend(decomposed.declarations);
                        class_name_fallback.extend(decomposed.fallback);
                        consumed.extend(decomposed.consumed);
                    }
                    _ => {}
                }
            }
            // Opaque, like a className-guard condition: never evaluated,
            // just threaded through by span for a later codegen stage to
            // re-emit verbatim. `disabled` below additionally has a static
            // shorthand form, which needs no source expression span.
            "onPress" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.on_press = Some(to_expr_ref(container.expression.span()));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "onStartShouldSetResponder" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_start_should_set_responder, &mut props.passthrough, scope, diagnostics, consumed),
            "onStartShouldSetResponderCapture" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_start_should_set_responder_capture, &mut props.passthrough, scope, diagnostics, consumed),
            "onMoveShouldSetResponder" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_move_should_set_responder, &mut props.passthrough, scope, diagnostics, consumed),
            "onMoveShouldSetResponderCapture" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_move_should_set_responder_capture, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderGrant" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_grant, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderStart" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_start, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderMove" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_move, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderEnd" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_end, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderRelease" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_release, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderReject" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_reject, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderTerminate" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_terminate, &mut props.passthrough, scope, diagnostics, consumed),
            "onResponderTerminationRequest" if matches!(primitive, Primitive::View | Primitive::Pressable) => capture_prop_expr(attr, &mut props.on_responder_termination_request, &mut props.passthrough, scope, diagnostics, consumed),
            "testID" => capture_prop_expr(attr, &mut props.test_id, &mut props.passthrough, scope, diagnostics, consumed),
            "nativeID" => capture_prop_expr(attr, &mut props.native_id, &mut props.passthrough, scope, diagnostics, consumed),
            "pointerEvents" => capture_prop_expr(attr, &mut props.pointer_events, &mut props.passthrough, scope, diagnostics, consumed),
            "accessibilityState" => {
                props.accessibility_state_keys = object_literal_keys(attr);
                capture_prop_expr(attr, &mut props.accessibility_state, &mut props.passthrough, scope, diagnostics, consumed)
            }
            "accessibilityValue" => capture_prop_expr(attr, &mut props.accessibility_value, &mut props.passthrough, scope, diagnostics, consumed),
            "accessibilityLiveRegion" => capture_prop_expr(attr, &mut props.accessibility_live_region, &mut props.passthrough, scope, diagnostics, consumed),
            "focusable" => match &attr.value {
                None => props.focusable = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.focusable = Some(match &container.expression {
                        // Read as the constant it is, so `focusable={false}`
                        // becomes `tabIndex={-1}` rather than a ternary over
                        // a literal -- and so the disabled check can tell it
                        // from an expression it cannot see into.
                        JSXExpression::BooleanLiteral(literal) => ConditionExpr::Static(literal.value),
                        expression => ConditionExpr::Ref(to_expr_ref(expression.span())),
                    });
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "onLayout" => capture_prop_expr(attr, &mut props.on_layout, &mut props.passthrough, scope, diagnostics, consumed),
            "level" if primitive == Primitive::Heading => match &attr.value {
                Some(JSXAttributeValue::StringLiteral(literal)) => {
                    props.heading_level = literal.value.parse::<u8>().ok()
                        .filter(|level| (1..=6).contains(level))
                        .map(HeadingLevel::Static);
                    if props.heading_level.is_none() {
                        props.passthrough.push(passthrough_prop(attr, scope, diagnostics, consumed));
                    }
                }
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.heading_level = match &container.expression {
                        JSXExpression::NumericLiteral(literal)
                            if literal.value.fract() == 0.0 && (1.0..=6.0).contains(&literal.value) =>
                            Some(HeadingLevel::Static(literal.value as u8)),
                        _ => Some(HeadingLevel::Dynamic(to_expr_ref(container.expression.span()))),
                    };
                }
                _ => props.passthrough.push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "ordered" if primitive == Primitive::List => match &attr.value {
                None => props.list_ordered = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.list_ordered = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props.passthrough.push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "onScroll" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => {
                capture_prop_expr(attr, &mut props.on_scroll, &mut props.passthrough, scope, diagnostics, consumed)
            }
            "scrollEventThrottle" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => {
                capture_prop_expr(attr, &mut props.scroll_event_throttle, &mut props.passthrough, scope, diagnostics, consumed)
            }
            "disabled" => match &attr.value {
                None => props.disabled = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.disabled = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "horizontal" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => match &attr.value {
                None => props.scroll_horizontal = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.scroll_horizontal = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "refreshing" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => match &attr.value {
                None => props.refreshing = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.refreshing = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "onRefresh" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.on_refresh = Some(to_expr_ref(container.expression.span()));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "keyboardShouldPersistTaps" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.keyboard_should_persist_taps = Some(to_expr_ref(container.expression.span()));
                }
                Some(JSXAttributeValue::StringLiteral(literal)) => {
                    props.keyboard_should_persist_taps = Some(to_expr_ref(literal.span));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "showsVerticalScrollIndicator" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => match &attr.value {
                None => props.shows_vertical_scroll_indicator = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.shows_vertical_scroll_indicator = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "showsHorizontalScrollIndicator" if matches!(primitive, Primitive::ScrollView | Primitive::FlatList) => match &attr.value {
                None => props.shows_horizontal_scroll_indicator = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.shows_horizontal_scroll_indicator = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            // Both spellings are accepted and neither is passed through:
            // the two platforms name this prop differently, so the value is
            // captured here and each backend writes it under its own name.
            // Re-emitting the source spelling verbatim would put
            // `accessibilityLabel` on a DOM `<input>`, where React drops it
            // and the field ends up with no accessible name at all -- the
            // exact failure the diagnostic exists to prevent.
            //
            // The *value* is never touched. Hozo diagnoses the absence of
            // a name and never invents or rewrites one: a name guessed from
            // a placeholder or a nearby heading is how a field comes to be
            // announced as something it isn't, which is worse than being
            // announced as nothing.
            "accessibilityLabel" | "aria-label" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.accessibility_label = Some(to_expr_ref(container.expression.span()));
                }
                Some(JSXAttributeValue::StringLiteral(literal)) => {
                    props.accessibility_label = Some(to_expr_ref(literal.span));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "accessibilityHint" | "aria-description" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.accessibility_hint = Some(to_expr_ref(container.expression.span()));
                }
                Some(JSXAttributeValue::StringLiteral(literal)) => {
                    props.accessibility_hint = Some(to_expr_ref(literal.span));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "src" if primitive == Primitive::Image => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.image_src = Some(to_expr_ref(container.expression.span()));
                }
                Some(JSXAttributeValue::StringLiteral(literal)) => {
                    props.image_src = Some(to_expr_ref(literal.span));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "defaultSource" if primitive == Primitive::Image => {
                capture_prop_expr(attr, &mut props.image_default_source, &mut props.passthrough, scope, diagnostics, consumed)
            }
            "alt" if primitive == Primitive::Image => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.accessibility_label = Some(to_expr_ref(container.expression.span()));
                }
                Some(JSXAttributeValue::StringLiteral(literal)) => {
                    props.accessibility_label = Some(to_expr_ref(literal.span));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "open" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.open = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
            },
            "onClose" => {
                props.has_on_close = true;
                props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed));
            }
            "placeholder" => {
                props.has_placeholder = true;
                props
                    .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed));
            }
            "role" | "accessibilityRole" => {
                props.accessibility_role =
                    accessibility_role_from_value(&attr.value, attr_name.name.as_str() == "role");
                if props.accessibility_role.is_none() {
                    // A dynamic/unrecognized role isn't modeled, but must
                    // still reach the output -- Hozo only declines to
                    // *reason* about it, not to emit it.
                    props
                        .passthrough
                        .push(passthrough_prop(attr, scope, diagnostics, consumed));
                }
            }
            _ => props
                .passthrough
                    .push(passthrough_prop(attr, scope, diagnostics, consumed)),
        }
    }

    // Every child, in source order. Anything the compiler doesn't model
    // becomes `Child::Verbatim` and is re-emitted from source rather than
    // dropped -- an unmodeled component, an expression container, a
    // fragment, a child spread.
    let mut children: Vec<Child> = Vec::new();
    for child in &el.children {
        match child {
            JSXChild::Element(child_el) => {
                match build_node(child_el, scope, diagnostics, consumed) {
                    Some(child_node) => children.push(Child::Node(child_node)),
                    // A component Hozo doesn't model still renders, and
                    // still occupies a position among its siblings.
                    None => children.push(carry_verbatim(
                        child,
                        child_el.span(),
                        scope,
                        diagnostics,
                        consumed,
                    )),
                }
            }
            JSXChild::Text(t) => {
                let cleaned = clean_jsx_text(t.value.as_str());
                if !cleaned.is_empty() {
                    children.push(Child::Text(cleaned));
                }
            }
            other => children.push(carry_verbatim(
                other,
                other.span(),
                scope,
                diagnostics,
                consumed,
            )),
        }
    }

    validate_semantic_children(primitive, &children, diagnostics);
    validate_focusable_disabled(&props, to_span(el.span()), diagnostics);

    Some(Node {
        primitive,
        style,
        props,
        children,
        class_name_fallback,
        carried_classes,
        span: to_span(el.span()),
    })
}

fn validate_semantic_children(
    parent: Primitive,
    children: &[Child],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for child in children {
        let Child::Node(child) = child else { continue };
        let allowed = match parent {
            Primitive::Paragraph | Primitive::Heading => matches!(
                child.primitive,
                Primitive::Text | Primitive::Link | Primitive::Button | Primitive::TextInput | Primitive::Image
            ),
            Primitive::List => child.primitive == Primitive::ListItem,
            _ => true,
        };
        if allowed {
            continue;
        }
        let parent_name = match parent {
            Primitive::Paragraph => "Paragraph",
            Primitive::Heading => "Heading",
            Primitive::List => "List",
            _ => continue,
        };
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::InvalidSemanticNesting,
            severity: Severity::Warning,
            message: format!(
                "`{parent_name}` cannot directly contain `{:?}` in semantic HTML. Move or wrap that child so the generated document structure remains valid.",
                child.primitive
            ),
            span: child.span,
        });
    }
}

/// Collects every top-level (i.e. not nested inside another already-visited
/// JSX element) `Node` tree found while walking a `Program`.
/// A top-level JSX element, plus where a hook declaration for it could go.
pub struct Root {
    pub node: Node,
    /// Byte offset just inside the enclosing function's opening `{`, where
    /// a generated `const x = useSomething()` can be spliced.
    ///
    /// `None` when there is nowhere to put one -- JSX at module scope, or
    /// inside a concise arrow body. Conditions that need a hook must be
    /// refused there rather than compiled into something invalid.
    ///
    /// A statement is the only safe position for these. Calling a hook
    /// inline in the JSX (`style={[a, useDark() && b]}`) looks tempting and
    /// breaks the rules of hooks as soon as the element itself sits behind
    /// a conditional -- the call order then changes between renders, which
    /// React treats as a hard error.
    pub hook_slot: Option<u32>,
}

pub struct JsxCollector<'r, 'a> {
    pub roots: Vec<Root>,
    /// Source-level diagnostics (things true of the written JSX itself,
    /// independent of which backend it later lowers to) -- as opposed to
    /// the lowering-level ones each backend raises during `lower()`.
    pub diagnostics: Vec<Diagnostic>,
    /// See `dynamic_class::Decomposed::consumed`.
    pub consumed: Vec<SourceSpan>,
    /// The innermost enclosing function body's insertion point, maintained
    /// as the walk descends. See `Root::hook_slot`.
    hook_slot: Option<u32>,
    scope: &'r Scope<'r, 'a>,
}

impl<'r, 'a> JsxCollector<'r, 'a> {
    pub fn new(scope: &'r Scope<'r, 'a>) -> Self {
        Self {
            roots: Vec::new(),
            diagnostics: Vec::new(),
            consumed: Vec::new(),
            hook_slot: None,
            scope,
        }
    }

    /// Runs `body` with `slot` as the current innermost function body,
    /// restoring the previous one afterwards. Nested functions therefore
    /// shadow their parent, which is what a hook needs: it belongs to the
    /// function that actually renders the JSX.
    fn within_function<F: FnOnce(&mut Self)>(&mut self, slot: Option<u32>, body: F) {
        let outer = self.hook_slot;
        self.hook_slot = slot;
        body(self);
        self.hook_slot = outer;
    }
}

impl<'r, 'a> Visit<'a> for JsxCollector<'r, 'a> {
    fn visit_jsx_element(&mut self, it: &JSXElement<'a>) {
        if let Some(node) = build_node(it, self.scope, &mut self.diagnostics, &mut self.consumed) {
            // Deliberately does not walk: `build_node` already recursed
            // into the children itself, so falling through to the generic
            // walker would visit (and re-collect) them a second time.
            self.roots.push(Root { node, hook_slot: self.hook_slot });
        } else {
            // But when nothing was built, nothing was visited either -- and
            // an element Hozo does not model is a boundary, not a wall.
            //
            // `<Card><View className="p-4"/></Card>` used to compile to
            // nothing at all: the outermost element is someone else's
            // component, so no root was collected, and the walk stopped
            // there rather than looking inside. Every Hozo primitive under
            // any wrapper the author wrote silently fell back to the
            // runtime components -- working, but with the compiler's whole
            // contribution quietly absent. Passing children into your own
            // component is ordinary React, not an edge case.
            //
            // The sibling form already worked (`<><Card/><View/></>`
            // collects the View), which is the same behaviour this gives
            // the nested one.
            walk_jsx_element(self, it);
        }
    }

    fn visit_function(&mut self, it: &Function<'a>, flags: ScopeFlags) {
        let slot = it.body.as_ref().map(|body| body.span.start + 1);
        self.within_function(slot, |this| walk_function(this, it, flags));
    }

    fn visit_arrow_function_expression(&mut self, it: &ArrowFunctionExpression<'a>) {
        // A concise body (`() => <View/>`) is an expression, not a block,
        // so there is no statement position to splice into. Reported as
        // "no slot" rather than silently producing invalid code.
        let slot = match &it.body {
            oxc_ast::ast::ArrowFunctionBody::FunctionBody(body) => Some(body.span.start + 1),
            _ => None,
        };
        self.within_function(slot, |this| walk_arrow_function_expression(this, it));
    }
}

#[cfg(test)]
mod tests {
    use hozo_ir::{AccessibilityRole, ConditionExpr, DiagnosticCode};

    /// Slices `source` at a passthrough prop's span, so tests assert on the
    /// text that will actually be re-emitted rather than raw offsets.
    fn passthrough_texts<'a>(source: &'a str, node: &hozo_ir::Node) -> Vec<&'a str> {
        node.props
            .passthrough
            .iter()
            .map(|p| &source[p.span.0.start as usize..p.span.0.end as usize])
            .collect()
    }

    #[test]
    fn unmodeled_props_and_spreads_are_preserved_verbatim() {
        let source = r#"
            import { View } from '@hozo/core'
            const el = <View {...rest} className="p-4" onLayout={onLayout} testID="row" />
            "#;
        let output = crate::parse_tsx(source);
        let root = &output.roots[0].node;
        assert_eq!(
            passthrough_texts(source, root),
            vec!["{...rest}"]
        );
        assert!(root.props.passthrough[0].is_spread);
        assert!(root.props.on_layout.is_some());
        assert!(root.props.test_id.is_some());
    }

    #[test]
    fn primitives_inside_a_render_prop_are_retained_for_nested_lowering() {
        let source = r#"
            import { FlatList, Text } from '@hozo/core'
            const el = <FlatList data={rows} renderItem={({ item }) => <Text className="p-2">{item}</Text>} />
            "#;
        let output = crate::parse_tsx(source);
        let render_item = output.roots[0]
            .node
            .props
            .passthrough
            .iter()
            .find(|prop| {
                let text = &source[prop.span.0.start as usize..prop.span.0.end as usize];
                text.starts_with("renderItem")
            })
            .expect("renderItem should carry its nested primitive");
        assert_eq!(render_item.nested.len(), 1);
        assert_eq!(render_item.nested[0].node.primitive, hozo_ir::Primitive::Text);
    }

    #[test]
    fn spread_after_class_name_is_diagnosed() {
        let output = crate::parse_tsx(
            r#"
            import { View } from '@hozo/core'
            const el = <View className="p-4" {...rest} />
            "#,
        );
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, DiagnosticCode::UnsafePropSpreadAfterStyle);
        // Still emitted despite the warning -- dropping it would be worse.
        assert_eq!(output.roots[0].node.props.passthrough.len(), 1);
    }

    #[test]
    fn spread_before_class_name_is_not_diagnosed() {
        // Safe ordering: className comes last, so it wins under JSX's
        // last-wins duplicate resolution -- nothing to warn about.
        let output = crate::parse_tsx(
            r#"
            import { View } from '@hozo/core'
            const el = <View {...rest} className="p-4" />
            "#,
        );
        assert!(output.diagnostics.is_empty());
        assert_eq!(output.roots[0].node.props.passthrough.len(), 1);
    }

    #[test]
    fn boolean_shorthand_disabled_is_a_static_true_condition() {
        // No expression to take a span from, so it can't become a
        // ConditionExpr -- but it must still reach the output.
        let source = r#"
            import { Button } from '@hozo/core'
            const el = <Button disabled>Save</Button>
            "#;
        let output = crate::parse_tsx(source);
        let root = &output.roots[0].node;
        assert_eq!(root.props.disabled, Some(ConditionExpr::Static(true)));
        assert!(passthrough_texts(source, root).is_empty());
    }

    #[test]
    fn dynamic_accessibility_role_reaches_output_even_though_it_is_not_modeled() {
        let source = r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable accessibilityRole={computedRole}>Go</Pressable>
            "#;
        let output = crate::parse_tsx(source);
        let root = &output.roots[0].node;
        assert_eq!(root.props.accessibility_role, None);
        assert_eq!(passthrough_texts(source, root), vec!["accessibilityRole={computedRole}"]);
    }

    #[test]
    fn invalid_semantic_nesting_is_diagnosed_without_guessing_through_expressions() {
        let source = r#"
            import { Paragraph, Section } from '@hozo/core'
            const el = <Paragraph>Intro<Section>Details</Section>{extra}</Paragraph>
            "#;
        let output = crate::parse_tsx(source);
        let semantic: Vec<_> = output.diagnostics.iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::InvalidSemanticNesting)
            .collect();
        assert_eq!(semantic.len(), 1);
        assert!(semantic[0].message.contains("Section"));
    }

    #[test]
    fn list_requires_static_direct_children_to_be_list_items() {
        let source = r#"
            import { List, ListItem, Paragraph } from '@hozo/core'
            const el = <List><ListItem>Good</ListItem><Paragraph>Bad</Paragraph>{items.map(render)}</List>
            "#;
        let output = crate::parse_tsx(source);
        let semantic: Vec<_> = output.diagnostics.iter()
            .filter(|diagnostic| diagnostic.code == DiagnosticCode::InvalidSemanticNesting)
            .collect();
        assert_eq!(semantic.len(), 1);
        assert!(semantic[0].message.contains("Paragraph"));
    }

    #[test]
    fn parses_on_press_and_accessibility_role() {
        let output = crate::parse_tsx(
            r#"
            import { Pressable } from '@hozo/core'
            const el = (
              <Pressable onPress={handlePress} accessibilityRole="button">
                Go
              </Pressable>
            )
            "#,
        );
        let root = &output.roots[0].node;
        assert!(root.props.on_press.is_some());
        assert_eq!(root.props.accessibility_role, Some(AccessibilityRole::Button));
    }

    #[test]
    fn parses_disabled_as_an_opaque_condition_expr() {
        let output = crate::parse_tsx(
            r#"
            import { Button } from '@hozo/core'
            const el = <Button disabled={isLoading}>Save</Button>
            "#,
        );
        let root = &output.roots[0].node;
        assert!(matches!(root.props.disabled, Some(ConditionExpr::Ref(_))));
    }

    #[test]
    fn accessibility_role_link_is_recognized() {
        let output = crate::parse_tsx(
            r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable accessibilityRole="link">Home</Pressable>
            "#,
        );
        assert_eq!(output.roots[0].node.props.accessibility_role, Some(AccessibilityRole::Link));
    }

    #[test]
    fn dynamic_accessibility_role_is_not_recognized() {
        // Conservative on purpose (see `accessibility_role_from_value`'s
        // doc comment): a role Hozo can't verify statically is treated as
        // absent, so the interactive-without-role diagnostic can still
        // fire rather than being silently suppressed by an unknown value.
        let output = crate::parse_tsx(
            r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable accessibilityRole={computedRole}>Go</Pressable>
            "#,
        );
        assert_eq!(output.roots[0].node.props.accessibility_role, None);
    }
}
