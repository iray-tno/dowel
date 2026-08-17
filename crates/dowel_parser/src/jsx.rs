//! Converts oxc's JSX AST into `dowel_ir::Node` trees.
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

use dowel_ir::{
    AccessibilityRole, Child, ConditionExpr, Diagnostic, DiagnosticCode, ExprRef, NestedNode, Node,
    PassthroughProp, Primitive, PropSet, Severity, SourceSpan, StyleDeclaration,
};
use oxc_ast::ast::{
    ArrowFunctionExpression, Function, JSXAttributeItem, JSXAttributeName, JSXAttributeValue,
    JSXChild, JSXElement, JSXElementName, JSXExpression,
};
use oxc_ast_visit::walk::{walk_arrow_function_expression, walk_function};
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

/// Finds and lowers Dowel primitives nested inside something the compiler
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
    module_record: &'r ModuleRecord<'a>,
    diagnostics: &'d mut Vec<Diagnostic>,
    consumed: &'d mut Vec<SourceSpan>,
    nested: Vec<NestedNode>,
}

impl<'r, 'a, 'd> Visit<'a> for PrimitiveFinder<'r, 'a, 'd> {
    fn visit_jsx_element(&mut self, it: &JSXElement<'a>) {
        if let JSXElementName::IdentifierReference(ident) = &it.opening_element.name {
            if let Some(name) = primitive_name(ident.name.as_str()) {
                match build_node(it, self.module_record, self.diagnostics, self.consumed) {
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
/// TypeScript do so Dowel's output says what the source said.
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

fn primitive_name(name: &str) -> Option<&'static str> {
    match name {
        "View" => Some("View"),
        "Text" => Some("Text"),
        "Pressable" => Some("Pressable"),
        "Button" => Some("Button"),
        "Link" => Some("Link"),
        "TextInput" => Some("TextInput"),
        "Dialog" => Some("Dialog"),
        _ => None,
    }
}

/// Builds a `Child::Verbatim` for a child the compiler carries rather than
/// reads, lowering any Dowel primitives nested inside it.
///
/// The expression is opaque; the primitives in it are not. So `show &&` is
/// left alone while the `<Text>` beside it compiles normally.
fn carry_verbatim(
    child: &JSXChild,
    span: Span,
    module_record: &ModuleRecord,
    diagnostics: &mut Vec<Diagnostic>,
    consumed: &mut Vec<SourceSpan>,
) -> Child {
    let mut finder = PrimitiveFinder {
        module_record,
        diagnostics,
        consumed,
        nested: Vec::new(),
    };
    finder.visit_jsx_child(child);
    Child::Verbatim { source: to_expr_ref(span), nested: finder.nested }
}

fn primitive_for_name(name: &str) -> Option<Primitive> {
    match name {
        "View" => Some(Primitive::View),
        "Text" => Some(Primitive::Text),
        "Pressable" => Some(Primitive::Pressable),
        "TextInput" => Some(Primitive::TextInput),
        "Dialog" => Some(Primitive::Dialog),
        "Button" => Some(Primitive::Button),
        "Link" => Some(Primitive::Link),
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
                            // Several groups only for a shorthand like
                            // `container`, which is a width plus a
                            // max-width at each breakpoint.
                            let groups = tailwind::expand_class(token);
                            let properties: Vec<_> =
                                groups.iter().flat_map(|(_, p)| p.clone()).collect();
                            // Reported only for brackets. An unknown bare
                            // class is ordinary -- projects have their own
                            // CSS and Dowel leaves it alone -- but a
                            // bracket is unambiguously Tailwind being
                            // asked for something, so failing to read one
                            // is worth saying out loud. It stayed silent
                            // until 2026-08-16, which is how `w-[32px]`
                            // came to compile to nothing at all.
                            if properties.is_empty() && tailwind::is_arbitrary(token) {
                                diagnostics.push(Diagnostic {
                                    code: DiagnosticCode::UnreadableArbitraryValue,
                                    severity: Severity::Warning,
                                    message: format!(
                                        "`{token}` uses Tailwind's arbitrary syntax and Dowel \
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
            // re-emit verbatim. `disabled` below additionally has a static
            // shorthand form, which needs no source expression span.
            "onPress" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.on_press = Some(to_expr_ref(container.expression.span()));
                }
                _ => props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
            },
            "disabled" => match &attr.value {
                None => props.disabled = Some(ConditionExpr::Static(true)),
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.disabled = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
            },
            // Both spellings are accepted and neither is passed through:
            // the two platforms name this prop differently, so the value is
            // captured here and each backend writes it under its own name.
            // Re-emitting the source spelling verbatim would put
            // `accessibilityLabel` on a DOM `<input>`, where React drops it
            // and the field ends up with no accessible name at all -- the
            // exact failure the diagnostic exists to prevent.
            //
            // The *value* is never touched. Dowel diagnoses the absence of
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
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
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
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
            },
            "open" => match &attr.value {
                Some(JSXAttributeValue::ExpressionContainer(container)) => {
                    props.open = Some(ConditionExpr::Ref(to_expr_ref(container.expression.span())));
                }
                _ => props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false }),
            },
            "onClose" => {
                props.has_on_close = true;
                props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false });
            }
            "placeholder" => {
                props.has_placeholder = true;
                props
                    .passthrough
                    .push(PassthroughProp { span: to_expr_ref(attr.span()), is_spread: false });
            }
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

    // Every child, in source order. Anything the compiler doesn't model
    // becomes `Child::Verbatim` and is re-emitted from source rather than
    // dropped -- an unmodeled component, an expression container, a
    // fragment, a child spread.
    let mut children: Vec<Child> = Vec::new();
    for child in &el.children {
        match child {
            JSXChild::Element(child_el) => {
                match build_node(child_el, module_record, diagnostics, consumed) {
                    Some(child_node) => children.push(Child::Node(child_node)),
                    // A component Dowel doesn't model still renders, and
                    // still occupies a position among its siblings.
                    None => children.push(carry_verbatim(
                        child,
                        child_el.span(),
                        module_record,
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
                module_record,
                diagnostics,
                consumed,
            )),
        }
    }

    Some(Node {
        primitive,
        style,
        props,
        children,
        class_name_fallback,
        span: to_span(el.span()),
    })
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
    module_record: &'r ModuleRecord<'a>,
}

impl<'r, 'a> JsxCollector<'r, 'a> {
    pub fn new(module_record: &'r ModuleRecord<'a>) -> Self {
        Self {
            roots: Vec::new(),
            diagnostics: Vec::new(),
            consumed: Vec::new(),
            hook_slot: None,
            module_record,
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
        // Deliberately does not call `walk_jsx_element` -- `build_node`
        // already recurses into children itself, so falling through to the
        // generic walker here would visit (and re-collect) nested elements
        // a second time.
        if let Some(node) = build_node(it, self.module_record, &mut self.diagnostics, &mut self.consumed) {
            self.roots.push(Root { node, hook_slot: self.hook_slot });
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
        let root = &output.roots[0].node;
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
        assert_eq!(output.roots[0].node.props.passthrough.len(), 1);
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
        assert_eq!(output.roots[0].node.props.passthrough.len(), 1);
    }

    #[test]
    fn boolean_shorthand_disabled_is_a_static_true_condition() {
        // No expression to take a span from, so it can't become a
        // ConditionExpr -- but it must still reach the output.
        let source = r#"
            import { Button } from '@dowel/core'
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
            import { Pressable } from '@dowel/core'
            const el = <Pressable accessibilityRole={computedRole}>Go</Pressable>
            "#;
        let output = crate::parse_tsx(source);
        let root = &output.roots[0].node;
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
        let root = &output.roots[0].node;
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
        let root = &output.roots[0].node;
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
        assert_eq!(output.roots[0].node.props.accessibility_role, Some(AccessibilityRole::Link));
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
        assert_eq!(output.roots[0].node.props.accessibility_role, None);
    }
}
