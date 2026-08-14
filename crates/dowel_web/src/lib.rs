//! Dowel IR to DOM/CSS/ARIA lowering (Web backend).
//!
//! Emits per-node scoped CSS classes rather than deduplicating/reusing
//! atomic utility classes across call sites -- a constraint carried over
//! from the cascade-ordering design discussion, since it's what lets
//! simple source-order-within-condition flattening (see `css.rs`) stay
//! correct without needing RNW/StyleX-style explicit priority tables.

mod css;
mod markup;

use dowel_ir::{Diagnostic, Node, Primitive};
use dowel_parser::ScannedUtility;

pub struct LowerOutput {
    pub jsx: String,
    pub css: String,
    pub diagnostics: Vec<Diagnostic>,
}

/// The proposal §8.1 "dowel-view" shared base style: applied to every
/// `View`, emitted once as a shared rule rather than duplicated per node.
const VIEW_BASE_CSS: &str = ".dowel-view {\n  \
    display: flex;\n  \
    flex-direction: column;\n  \
    flex-shrink: 0;\n  \
    position: relative;\n  \
    min-width: 0;\n  \
    box-sizing: border-box;\n\
}\n\n";

struct ClassAllocator {
    next: u32,
}

impl ClassAllocator {
    fn alloc(&mut self) -> String {
        let name = format!("dowel-{}", self.next);
        self.next += 1;
        name
    }
}

/// `source` is the original TSX text `root` was parsed from -- needed to
/// re-emit `ExprRef`/`ConditionExpr` guards verbatim (they're spans into
/// it, not evaluated by the compiler; see `dowel_ir`'s doc comments).
///
/// `scanned` is the source's candidate classes (`dowel_parser::
/// scan_class_candidates`). They're only emitted when the tree actually
/// has a `className` that couldn't be resolved statically -- see
/// `render_fallback_utilities`.
pub fn lower(root: &Node, source: &str, scanned: &[ScannedUtility]) -> LowerOutput {
    let mut allocator = ClassAllocator { next: 0 };
    let mut rules = String::new();
    let mut diagnostics = Vec::new();
    let mut uses_view_base = false;

    let jsx = render_node(root, source, &mut allocator, &mut rules, &mut diagnostics, &mut uses_view_base);

    let mut css = String::new();
    if uses_view_base {
        css.push_str(VIEW_BASE_CSS);
    }
    // An `animation` declaration is inert without its `@keyframes`, and
    // those are document-level rather than per-node -- so they're collected
    // across the whole tree and emitted once, deduplicated.
    for keyframes in collect_keyframes(root) {
        css.push_str(keyframes);
        css.push_str("\n\n");
    }
    css.push_str(&render_fallback_utilities(root, scanned));
    css.push_str(&rules);

    LowerOutput { jsx, css, diagnostics }
}

/// CSS for scanned candidate classes, under their real Tailwind names.
///
/// Only emitted when some `className` in the tree couldn't be resolved
/// statically. Everything Dowel *could* read is already compiled into
/// scoped `.dowel-N` rules, so emitting these unconditionally would ship
/// a second, redundant copy of the same styles on every file.
///
/// This is what makes proposal §7's third tier actually do something: the
/// preserved expression evaluates to a class string at runtime, and now
/// there's a matching rule for the browser to apply. No runtime code is
/// involved -- the CSS engine does the resolution.
fn render_fallback_utilities(root: &Node, scanned: &[ScannedUtility]) -> String {
    if scanned.is_empty() || !has_class_name_fallback(root) {
        return String::new();
    }
    let mut out = String::new();
    for utility in scanned {
        let selector = css::escape_class_selector(&utility.class_name);
        out.push_str(&css::render_rule(&selector, &utility.condition, &utility.properties));
        out.push_str("\n\n");
    }
    out
}

fn has_class_name_fallback(node: &Node) -> bool {
    !node.class_name_fallback.is_empty() || node.children.iter().any(has_class_name_fallback)
}

/// Every distinct `@keyframes` block the tree's animations need, in
/// first-use order so output stays deterministic.
fn collect_keyframes(node: &Node) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    collect_keyframes_into(node, &mut found);
    found
}

fn collect_keyframes_into(node: &Node, found: &mut Vec<&'static str>) {
    for declaration in &node.style {
        if let dowel_ir::StyleProperty::Animation(animation) = declaration.property {
            if let Some(keyframes) = animation.keyframes() {
                if !found.contains(&keyframes) {
                    found.push(keyframes);
                }
            }
        }
    }
    for child in &node.children {
        collect_keyframes_into(child, found);
    }
}

/// Byte-slices `source` at an `ExprRef`'s span. Spans come from oxc's own
/// tokenizer over this same `source`, so they're always on UTF-8 character
/// boundaries -- not re-validated here.
fn source_text(source: &str, expr_ref: dowel_ir::ExprRef) -> &str {
    &source[expr_ref.0.start as usize..expr_ref.0.end as usize]
}

/// Re-emits a `ConditionExpr` as a JS boolean expression by splicing the
/// original source at each leaf `Ref`'s span -- the compiler never
/// evaluates these, only reconstructs them with real `&&`/`||`/`!`
/// wrapping the *combinator structure* it built (see dowel_parser's
/// `dynamic_class` module), not anything it parsed out of the leaves
/// themselves.
fn render_condition_expr(source: &str, expr: &dowel_ir::ConditionExpr) -> String {
    use dowel_ir::ConditionExpr;
    match expr {
        ConditionExpr::Ref(r) => source_text(source, *r).to_string(),
        ConditionExpr::Not(inner) => format!("!({})", render_condition_expr(source, inner)),
        ConditionExpr::And(a, b) => {
            format!("({}) && ({})", render_condition_expr(source, a), render_condition_expr(source, b))
        }
        ConditionExpr::Or(a, b) => {
            format!("({}) || ({})", render_condition_expr(source, a), render_condition_expr(source, b))
        }
    }
}

fn render_node(
    node: &Node,
    source: &str,
    allocator: &mut ClassAllocator,
    rules: &mut String,
    diagnostics: &mut Vec<Diagnostic>,
    uses_view_base: &mut bool,
) -> String {
    let class_name = allocator.alloc();

    for (condition, props) in dowel_ir::group_by_condition(&node.style) {
        let props = dowel_ir::dedupe_last_wins(props);
        if props.is_empty() {
            continue;
        }
        rules.push_str(&css::render_rule(&class_name, &condition, &props));
        rules.push_str("\n\n");
    }

    let (tag, extra_attrs) = markup::element_shape(node, diagnostics);

    let mut classes = class_name;
    if node.primitive == Primitive::View {
        *uses_view_base = true;
        classes = format!("dowel-view {classes}");
    }

    // `className`, not `class` -- Dowel's Web output is consumed as JSX
    // (the Vite plugin splices it back into React source), not raw HTML.
    //
    // Anything the parser couldn't decompose statically (proposal §7's
    // third tier) is concatenated back on at runtime. `lower` emits CSS
    // for every candidate class the scanner found in this file, so
    // whichever one the expression evaluates to has a real rule behind it
    // -- the browser's own CSS engine does the resolution, no runtime.
    let mut attrs = if node.class_name_fallback.is_empty() {
        format!(r#" className="{classes}""#)
    } else {
        for expr_ref in &node.class_name_fallback {
            diagnostics.push(dowel_ir::Diagnostic {
                code: dowel_ir::DiagnosticCode::DynamicClassNameNotResolved,
                severity: dowel_ir::Severity::Warning,
                message: format!(
                    "`{}` can't be resolved at build time, so it's passed through and its CSS \
                     comes from scanning this file for candidate classes. Only classes whose \
                     text appears in this file are covered -- one built from a variable, or \
                     coming from another module, still won't be.",
                    source_text(source, *expr_ref)
                ),
                span: node.span,
            });
        }
        let parts: Vec<String> = std::iter::once(format!(r#""{classes}""#))
            .chain(node.class_name_fallback.iter().map(|r| source_text(source, *r).to_string()))
            .collect();
        format!(" className={{[{}].filter(Boolean).join(' ')}}", parts.join(", "))
    };
    for (key, value) in &extra_attrs {
        attrs.push_str(&format!(r#" {key}="{value}""#));
    }

    if let Some(on_press) = node.props.on_press {
        attrs.push_str(&format!(" onClick={{{}}}", source_text(source, on_press)));
    }
    if let Some(disabled) = &node.props.disabled {
        // `disabled` is a real, React-boolean-aware HTML attribute only on
        // actual form controls (<button> here) -- react omits it entirely
        // when the value is falsy. Everything else Dowel maps to a <div>
        // (Pressable, View, Text), where the native attribute has no
        // effect at all, so ARIA is the honest choice there instead.
        let attr_name = if node.primitive == Primitive::Button { "disabled" } else { "aria-disabled" };
        attrs.push_str(&format!(" {attr_name}={{{}}}", render_condition_expr(source, disabled)));
    }

    // CSS attribute selectors (`[data-dowel-cond-x-y]`, built in css.rs)
    // match on an attribute's *presence*, not its string value -- so the
    // guard must be wired as `{expr ? '' : undefined}` (React omits
    // `undefined`-valued attributes entirely) rather than a literal
    // "true"/"false" string, which would stay present either way and
    // permanently match the selector.
    for expr_ref in collect_expr_refs(node) {
        let guard = source_text(source, expr_ref);
        attrs.push_str(&format!(" {}={{{guard} ? '' : undefined}}", css::expr_ref_attribute(expr_ref)));
    }

    // Everything Dowel doesn't model, re-emitted verbatim and last so JSX's
    // last-wins duplicate resolution keeps matching the source's own
    // ordering semantics. Emitted as written, including RN-specific props
    // (`testID`) that React DOM will warn about as unknown on Web -- a
    // visible warning beats silently dropping what the author wrote;
    // mapping those to Web equivalents is a separate piece of work.
    for prop in &node.props.passthrough {
        attrs.push(' ');
        attrs.push_str(source_text(source, prop.span));
    }

    let inner = match &node.text {
        Some(dowel_ir::TextContent::Literal(text)) => markup::html_escape(text),
        Some(dowel_ir::TextContent::Dynamic(_)) | None => node
            .children
            .iter()
            .map(|child| render_node(child, source, allocator, rules, diagnostics, uses_view_base))
            .collect(),
    };

    format!("<{tag}{attrs}>{inner}</{tag}>")
}

fn collect_expr_refs(node: &Node) -> Vec<dowel_ir::ExprRef> {
    let mut refs = Vec::new();
    for decl in &node.style {
        if let dowel_ir::Condition::Expr(expr) = &decl.condition {
            collect_from_expr(expr, &mut refs);
        }
    }
    refs.sort_by_key(|r: &dowel_ir::ExprRef| (r.0.start, r.0.end));
    refs.dedup();
    refs
}

fn collect_from_expr(expr: &dowel_ir::ConditionExpr, out: &mut Vec<dowel_ir::ExprRef>) {
    use dowel_ir::ConditionExpr;
    match expr {
        ConditionExpr::Ref(r) => out.push(*r),
        ConditionExpr::Not(inner) => collect_from_expr(inner, out),
        ConditionExpr::And(a, b) | ConditionExpr::Or(a, b) => {
            collect_from_expr(a, out);
            collect_from_expr(b, out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOGIN_EXAMPLE: &str = r#"
import { View, Text, Button } from '@dowel/core'

export function Login() {
  return (
    <View className="flex-1 items-center justify-center p-6">
      <Text className="text-xl font-bold">
        Welcome
      </Text>

      <Button className="mt-4 px-4 py-2">
        Continue
      </Button>
    </View>
  )
}
"#;

    #[test]
    fn lowers_the_login_example_to_html_and_css() {
        let parsed = dowel_parser::parse_tsx(LOGIN_EXAMPLE);
        let root = &parsed.roots[0];
        let output = lower(root, LOGIN_EXAMPLE, &[]);

        assert!(output.jsx.starts_with(r#"<div className="dowel-view dowel-0">"#));
        assert!(output.jsx.contains("<span className=\"dowel-1\">Welcome</span>"));
        assert!(output.jsx.contains("<button className=\"dowel-2\">Continue</button>"));

        assert!(output.css.contains(".dowel-view {"));
        assert!(output.css.contains(".dowel-0 {"));
        assert!(output.css.contains("flex: 1 1 0%;"));
        assert!(output.css.contains("padding-top: 24px;"));
        assert!(output.css.contains(".dowel-1 {"));
        assert!(output.css.contains("font-size: 20px;"));
        assert!(output.css.contains("font-weight: 700;"));
        assert!(output.css.contains(".dowel-2 {"));
        // `px-4` is Tailwind's logical inline axis, not left/right.
        assert!(output.css.contains("padding-inline-start: 16px;"));
        assert!(output.css.contains("padding-inline-end: 16px;"));

        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn hover_condition_compiles_to_a_real_pseudo_class() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="hover:text-xl" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.css.contains(".dowel-0:hover {"));
        assert!(output.css.contains("font-size: 20px;"));
    }

    #[test]
    fn an_unresolvable_class_name_is_preserved_not_dropped() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className={classNameFromProps} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);

        // The expression reaches the DOM instead of vanishing...
        assert!(output.jsx.contains("classNameFromProps"));
        // ...and the diagnostic says Dowel generates no CSS behind it,
        // rather than letting it look resolved.
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics[0].code,
            dowel_ir::DiagnosticCode::DynamicClassNameNotResolved
        );
    }

    #[test]
    fn scanned_candidates_give_an_unresolvable_class_name_real_css() {
        // The classes here live inside a function body -- exactly what the
        // AST-based reader can't see. Scanning finds them, and the CSS is
        // emitted under their real Tailwind names so the string the
        // expression produces at runtime actually matches something. No
        // runtime code involved: the browser's CSS engine resolves it.
        let source = r#"
            import { View } from '@dowel/core'
            function getDynamic(isWide) {
              return isWide ? 'p-8' : 'p-2'
            }
            const el = <View className={getDynamic(isWide)} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let scanned = dowel_parser::scan_class_candidates(source);
        let output = lower(&parsed.roots[0], source, &scanned);

        assert!(output.css.contains(".p-8 {"));
        assert!(output.css.contains(".p-2 {"));
        assert!(output.css.contains("padding-top: 32px;"));
    }

    #[test]
    fn scanned_candidates_are_not_emitted_without_a_fallback() {
        // Everything readable is already compiled into scoped rules, so
        // emitting these too would ship a second copy of the same styles
        // on every file.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-4" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let scanned = dowel_parser::scan_class_candidates(source);
        assert!(!scanned.is_empty(), "the scanner should still find p-4");

        let output = lower(&parsed.roots[0], source, &scanned);
        assert!(output.css.contains(".dowel-0 {"));
        assert!(!output.css.contains(".p-4 {"));
    }

    #[test]
    fn scanned_variant_classes_are_escaped_in_the_selector() {
        // `hover:bg-blue-500` contains selector syntax and has to be
        // written `.hover\:bg-blue-500:hover` to match literally.
        let source = r#"
            import { View } from '@dowel/core'
            const extra = 'hover:bg-blue-500'
            const el = <View className={extra} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let scanned = dowel_parser::scan_class_candidates(source);
        let output = lower(&parsed.roots[0], source, &scanned);
        assert!(output.css.contains(r".hover\:bg-blue-500:hover {"));
    }

    #[test]
    fn only_the_unresolvable_leaf_falls_back() {
        // proposal §7's three tiers in one className: a literal compiles
        // away, a guarded literal becomes a conditional rule, and only the
        // opaque call is passed through.
        let source = r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', active && 'text-xl', getDynamic())} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);

        assert!(output.css.contains("padding-top: 16px;"));
        assert!(output.css.contains("font-size: 20px;"));
        assert!(output.jsx.contains("getDynamic()"));
        // The parts that did compile aren't repeated in the fallback.
        assert!(!output.jsx.contains("'p-4'"));
    }

    #[test]
    fn space_x_becomes_a_child_scoped_rule() {
        // `space-*` is the one utility that styles the element's children
        // rather than the element, so it can't be a declaration on the
        // node's own rule.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="space-x-2" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.css.contains(":where(.dowel-0 > :not(:last-child)) {"));
        assert!(output.css.contains("margin-inline-end: 8px;"));
        // Not on the element itself.
        assert!(!output.css.contains(".dowel-0 {\n  margin-inline-end"));
    }

    #[test]
    fn animation_emits_its_keyframes_once() {
        let source = r#"
            import { View, Text } from '@dowel/core'
            const el = (
              <View className="animate-spin">
                <Text className="animate-spin">x</Text>
              </View>
            )
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.css.contains("animation: spin 1s linear infinite;"));
        // An `animation` declaration is inert without its keyframes, and
        // two users of the same animation must not duplicate the block.
        assert_eq!(output.css.matches("@keyframes spin").count(), 1);
    }

    #[test]
    fn unmodeled_props_and_spreads_reach_the_output() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-4" {...rest} onLayout={onLayout} testID="row" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.jsx.contains("{...rest}"));
        assert!(output.jsx.contains("onLayout={onLayout}"));
        assert!(output.jsx.contains(r#"testID="row""#));
    }

    #[test]
    fn pressed_condition_compiles_to_a_real_active_pseudo_class() {
        let source = r#"
            import { Button } from '@dowel/core'
            const el = <Button className="pressed:opacity-50">Save</Button>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.css.contains(".dowel-0:active {"));
        assert!(output.css.contains("opacity: 0.5;"));
    }

    #[test]
    fn interactive_pressable_without_role_is_diagnosed_from_real_source() {
        // Previously only reachable by hand-constructing a `Node` directly
        // -- `PropSet.on_press`/`accessibility_role` weren't populated by
        // the parser at all until dowel_parser::jsx gained onPress/
        // accessibilityRole attribute parsing.
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable onPress={handleTap}>Tap</Pressable>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::A11yInteractiveWithoutRole);
        assert!(!output.jsx.contains("role="));
        // onPress -> onClick is wired regardless of the diagnostic.
        assert!(output.jsx.contains("onClick={handleTap}"));
    }

    #[test]
    fn accessibility_role_suppresses_the_diagnostic_and_sets_role() {
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = (
              <Pressable onPress={handleTap} accessibilityRole="button">Tap</Pressable>
            )
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.diagnostics.is_empty());
        assert!(output.jsx.contains(r#"role="button""#));
    }

    #[test]
    fn disabled_renders_the_native_attribute_on_button() {
        let source = r#"
            import { Button } from '@dowel/core'
            const el = <Button disabled={isLoading}>Save</Button>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.jsx.contains("disabled={isLoading}"));
        assert!(!output.jsx.contains("aria-disabled"));
    }

    #[test]
    fn disabled_renders_aria_disabled_on_pressable() {
        // Pressable is a <div> -- the native `disabled` attribute has no
        // effect there, so this must be ARIA instead.
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable disabled={isLoading} accessibilityRole="button">Save</Pressable>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);
        assert!(output.jsx.contains("aria-disabled={isLoading}"));
    }

    #[test]
    fn dynamic_class_name_guard_is_wired_as_a_presence_toggle() {
        let source = r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', active && 'text-xl')} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source, &[]);

        // The guard is re-emitted verbatim, wired to toggle attribute
        // *presence* (not a literal "true"/"false" string, which would
        // permanently match the CSS attribute selector either way).
        assert!(output.jsx.contains("={active ? '' : undefined}"));
        assert!(!output.jsx.contains(r#"="false""#));
        assert!(!output.jsx.contains(r#"="true""#));

        // And the CSS selector that attribute name feeds is present too.
        assert!(output.css.contains("] {"));
    }
}
