//! Dowel IR to React Native primitive/StyleSheet lowering (Native backend).
//!
//! `Condition::Always` merges directly into the rendered `style` prop.
//! Other conditions merge too, each keyed to whatever real value drives
//! them -- `Disabled` uses `PropSet.disabled`'s guard (the *style*
//! condition itself carries no expression; the actual boolean comes from
//! the separate `disabled={...}` prop), `Expr` carries its own guard
//! directly. Both get spliced into a conditional `style={[base, guard &&
//! variant]}` array, re-emitting the guard verbatim from `source` (see
//! `render_condition_expr`) exactly like `dowel_web` does for its
//! attribute-toggle wiring -- same "never evaluate, only re-emit" rule.
//!
//! `Pressed` merges too, but differently: RN's `Pressable` already tracks
//! press state natively via a `style={({ pressed }) => [...]}` render-prop
//! form (no synthesized state needed, unlike what an earlier pass of this
//! design assumed) -- so a node with a `Pressed` condition gets its whole
//! `style` prop wrapped in that function instead of being a plain array.
//! Only applies when `component == "Pressable"` (Button maps to it too);
//! a function isn't a valid `style` value on View/Text, so `Pressed` stays
//! unmerged there, same treatment as Hover/Focus/Responsive below.
//!
//! `Hover`/`Focus`/`Responsive`/`Dark`/`FirstChild` still don't merge into
//! anything -- and until 2026-08-15 that was a **silent** drop, not the
//! "honest gap" an earlier version of this comment claimed. Their style
//! objects were computed into the StyleSheet and then never referenced by
//! the rendered JSX, with no diagnostic; the conformance suite scored all
//! eight variant candidates as covered because the entry existed. They now
//! report themselves (`DiagnosticCode::VariantNotWiredOnNative`), split by
//! whether dropping one renders the wrong thing:
//!
//! - `Responsive`/`Dark` are errors. Each has an ordinary React Native
//!   counterpart, and a tablet layout or a dark-mode appearance that
//!   silently doesn't apply is a real bug.
//! - `FirstChild` is *resolved*, not reported, whenever the compiler can
//!   see the element's position among its siblings -- which is most of the
//!   time, since it is looking straight at the JSX tree. Web asks
//!   `:first-child` at match time; here the answer is already known, so it
//!   costs nothing at runtime. Only an undecidable position (a component
//!   root, or a sibling of something unmodeled -- see
//!   `Node::children_complete`) is an error.
//! - `Hover`/`Focus` are warnings. Also unbuilt rather than impossible --
//!   a tablet with a trackpad or pencil reports hover, as do the
//!   macOS/Windows/visionOS targets -- but stopping a cross-platform build
//!   over a `hover:` written for Web would be worse than the gap.
//! - `Disabled` without a `disabled` prop, and `Pressed` on anything but a
//!   Pressable, are errors: nothing on the element can drive them.

mod markup;
mod style;

use dowel_ir::{
    Breakpoint, Condition, ConditionExpr, Diagnostic, DiagnosticCode, ExprRef, Node, Primitive,
    Severity, StyleDeclaration, StyleProperty, TextContent, TextOverflow, WhiteSpace,
};

pub struct LowerOutput {
    pub jsx: String,
    /// A `StyleSheet.create({ ... })`-ready JS object literal (without the
    /// `StyleSheet.create(` wrapper -- left to the caller, since whether/how
    /// to wrap and import `StyleSheet` is a codegen-site decision).
    pub styles: String,
    pub diagnostics: Vec<Diagnostic>,
}

struct NameAllocator {
    next: u32,
}

impl NameAllocator {
    fn alloc(&mut self) -> String {
        let name = format!("dowel{}", self.next);
        self.next += 1;
        name
    }
}

/// `source` is the original TSX text `root` was parsed from -- needed to
/// re-emit `ExprRef`/`ConditionExpr` guards verbatim (they're spans into
/// it, never evaluated by the compiler; see `dowel_ir`'s doc comments).
pub fn lower(root: &Node, source: &str) -> LowerOutput {
    let mut allocator = NameAllocator { next: 0 };
    let mut style_entries: Vec<(String, Vec<StyleProperty>)> = Vec::new();
    let mut diagnostics = Vec::new();

    // The root's position is genuinely unknowable here: it's whatever the
    // component's caller renders it into.
    let jsx = render_node(
        root,
        SiblingPosition::Unknown,
        source,
        &mut allocator,
        &mut style_entries,
        &mut diagnostics,
    );

    let mut styles = String::from("{\n");
    for (name, props) in &style_entries {
        styles.push_str(&format!("  {name}: {{\n"));
        for (key, value) in style_pairs(props) {
            styles.push_str(&format!("    {key}: {value},\n"));
        }
        styles.push_str("  },\n");
    }
    styles.push('}');

    LowerOutput { jsx, styles, diagnostics }
}

/// The Native counterpart of `dowel_web::render_candidate_stylesheet`:
/// the module that lets a `className` the compiler couldn't read still
/// produce styles (proposal §7's third tier).
///
/// The two platforms need very different amounts of machinery here, and
/// the reason is worth stating. On Web the candidate stylesheet is free --
/// the browser already *has* a CSS engine, so emitting rules costs bytes
/// and no code. React Native has no such engine, so something must turn a
/// class string into a style object on device.
///
/// This is deliberately the smallest thing that can: a flat
/// name -> style-object map plus a split-and-look-up resolver
/// (`@dowel/runtime`'s `createClassResolver`). What makes that enough,
/// where `react-native-css` needs a full reactive engine with specificity
/// sorting, is that Dowel only ever puts *single utility classes* in here.
/// They're all the same specificity, so "later in the string wins" is the
/// whole cascade -- which is exactly what React Native's own style-array
/// merging already does.
///
/// Conditional utilities (`hover:`, `md:`, `pressed:`) are the price. A
/// style object can't express them, and making it able to would mean
/// per-component state tracking -- i.e. rebuilding the engine this design
/// is choosing not to ship. They go into `unsupported` instead of being
/// dropped, so the resolver can say so at the moment one is actually used
/// rather than rendering silently wrong. A candidate merely *appearing* in
/// the scan proves nothing (it may only ever be used on Web, or in a
/// static `className` that compiled fine), which is why this is a runtime
/// warning and not a build error.
pub fn render_candidate_module(class_names: &[String]) -> String {
    let mut supported: Vec<(&String, Vec<(&'static str, String)>)> = Vec::new();
    let mut unsupported: Vec<(&String, String)> = Vec::new();

    for name in class_names {
        let Some(utility) = dowel_parser::resolve_class_name(name) else {
            continue;
        };
        if utility.condition != Condition::Always {
            unsupported.push((name, format!("`{name}` is conditional, and a runtime-resolved class can only carry unconditional styles on React Native. Write it as a static className so it compiles to a real style variant.")));
            continue;
        }
        if let Some(reason) =
            utility.properties.iter().find_map(|p| p.unsupported_on_native())
        {
            unsupported.push((name, format!("{reason} -- this utility is Web-only.")));
            continue;
        }
        let pairs = style_pairs(&utility.properties);
        if pairs.is_empty() {
            continue;
        }
        supported.push((name, pairs));
    }

    let mut out = String::from(
        "// Generated by Dowel. Do not edit.\n\
         import { createClassResolver } from '@dowel/runtime'\n\n\
         const styles = {\n",
    );
    for (name, pairs) in &supported {
        out.push_str(&format!("  {}: {{\n", quote(name)));
        for (key, value) in pairs {
            out.push_str(&format!("    {key}: {value},\n"));
        }
        out.push_str("  },\n");
    }
    out.push_str("}\n\nconst unsupported = {\n");
    for (name, reason) in &unsupported {
        out.push_str(&format!("  {}: {},\n", quote(name), quote(reason)));
    }
    out.push_str("}\n\nexport const dowelClasses = createClassResolver(styles, unsupported)\n");
    out
}

/// A JS string literal. Class names carry `:` `/` `[` `]` and reasons carry
/// backticks and apostrophes, so both are quoted rather than emitted bare.
fn quote(text: &str) -> String {
    let escaped = text.replace('\\', r"\\").replace('"', "\\\"").replace('\n', "\\n");
    format!("\"{escaped}\"")
}

/// The React Native `key: value` pairs a set of IR properties becomes.
///
/// Distinct IR properties can collapse onto one RN key (all four per-side
/// border styles map to `borderStyle`), which would emit a duplicate object
/// key. Keep the last, matching how JS itself would resolve it -- but
/// written once.
fn style_pairs(props: &[StyleProperty]) -> Vec<(&'static str, String)> {
    let mut emitted: Vec<(&'static str, String)> = Vec::new();
    for prop in props {
        for (key, value) in style::property_and_value(prop) {
            // A property refused for Native (see
            // `StyleProperty::unsupported_on_native`) yields no value;
            // writing the key anyway would emit `height: ,`, which isn't
            // even parseable JS.
            if value.is_empty() {
                continue;
            }
            match emitted.iter_mut().find(|(existing, _)| *existing == key) {
                Some(slot) => slot.1 = value,
                None => emitted.push((key, value)),
            }
        }
    }
    if let Some(transform) = style::transform_entry(props) {
        emitted.push(transform);
    }
    emitted
}

/// Byte-slices `source` at an `ExprRef`'s span. Spans come from oxc's own
/// tokenizer over this same `source`, so they're always on UTF-8 character
/// boundaries -- not re-validated here.
fn source_text(source: &str, expr_ref: ExprRef) -> &str {
    &source[expr_ref.0.start as usize..expr_ref.0.end as usize]
}

/// Re-emits a `ConditionExpr` as a JS boolean expression by splicing the
/// original source at each leaf `Ref`'s span, reconstructed with real
/// `&&`/`||`/`!` matching the combinator structure the compiler built
/// (see dowel_parser's `dynamic_class` module) -- never anything parsed
/// out of the leaves themselves.
fn render_condition_expr(source: &str, expr: &ConditionExpr) -> String {
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

/// Where a node sits among its siblings, as far as the compiler can tell.
///
/// This is what lets `first:` be resolved at build time instead of needing
/// a selector engine: CSS asks the question at match time, but the compiler
/// is looking at the JSX tree and usually already knows the answer. It's a
/// small example of the general shape -- a condition Web resolves at
/// runtime that Native can have for free by resolving it earlier.
///
/// `Unknown` is not a failure to compute; it's the honest answer whenever
/// the position genuinely isn't decidable here (see
/// `Node::children_complete`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SiblingPosition {
    First,
    NotFirst,
    Unknown,
}

fn render_node(
    node: &Node,
    position: SiblingPosition,
    source: &str,
    allocator: &mut NameAllocator,
    style_entries: &mut Vec<(String, Vec<StyleProperty>)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let base_name = allocator.alloc();
    let mut style_array_parts: Vec<String> = Vec::new();
    // Held separately from `style_array_parts` because they can only be
    // merged once `component` is known (below) -- RN's pressed-render-prop
    // form of `style` only exists on Pressable; on View/Text a function
    // isn't a valid style value at all, so it must not be used there.
    let mut pressed_parts: Vec<String> = Vec::new();

    // Web concatenates an unresolvable `className` back on and lets the
    // browser's CSS engine match it. React Native has no className and no
    // CSS engine, so the string is handed to the generated resolver
    // instead (see `render_candidate_module`), which looks each class up in
    // the project-wide candidate map. Warning rather than error: the styles
    // do arrive, but only for classes whose text appears literally
    // somewhere in the project and that aren't conditional.
    for expr_ref in &node.class_name_fallback {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::DynamicClassNameNotResolved,
            severity: Severity::Warning,
            message: format!(
                "`{}` can't be resolved at build time, so it's resolved on device from the \
                 project-wide candidate map. Conditional utilities (`hover:`, `md:`, `pressed:`) \
                 can't be carried that way and will warn at runtime -- write those as a static \
                 className so they compile to a real style variant.",
                source_text(source, *expr_ref)
            ),
            span: node.span,
        });
    }

    // Some CSS concepts are props on this platform rather than styles, so
    // they're absorbed before the refusal check below -- otherwise the
    // thing that *does* express them would be reported as impossible.
    let truncation = truncation_props(node);

    for declaration in &node.style {
        if truncation.is_some() && is_truncation_declaration(&declaration.property) {
            continue;
        }
        if let Some(reason) = truncation_only_reason(&declaration.property) {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::WebOnlyPropertyOnNative,
                severity: Severity::Error,
                message: reason,
                span: node.span,
            });
            continue;
        }
        // Refused rather than dropped: silently ignoring a `block`/`grid`/
        // `h-screen` would leave a layout that looks right on Web and is
        // wrong on device with nothing pointing at the cause.
        if let Some(reason) = declaration.property.unsupported_on_native() {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::WebOnlyPropertyOnNative,
                severity: Severity::Error,
                message: format!("{reason} -- this utility is Web-only."),
                span: node.span,
            });
        }
    }

    let (component, extra_props) = markup::native_component(node, diagnostics);

    // Only `Text` can hold text on this platform -- a raw string inside a
    // View or Pressable is a runtime crash there ("Text strings must be
    // rendered within a <Text> component"), while the same source is fine
    // on Web. So one is inserted. Its styles have to move with it: React
    // Native's Text inherits from an enclosing Text but *not* from a View,
    // so leaving `fontSize` on the parent would silently render at the
    // default size instead.
    let wraps_text = component != "Text" && node.text.is_some();
    let (text_declarations, own_declarations): (Vec<_>, Vec<_>) = if wraps_text {
        node.style.iter().cloned().partition(|d| is_text_property(&d.property))
    } else {
        (Vec::new(), node.style.to_vec())
    };

    build_style_entries(
        &own_declarations,
        &base_name,
        source,
        node,
        position,
        style_entries,
        &mut style_array_parts,
        &mut pressed_parts,
        diagnostics,
    );

    // After the compiled styles, so it wins the same way it would in the
    // source: `cn('p-4', getDynamic())` puts the opaque part last, and RN
    // resolves a style array last-wins just like JSX's own duplicate-prop
    // rule.
    for expr_ref in &node.class_name_fallback {
        style_array_parts.push(format!("dowelClasses({})", source_text(source, *expr_ref)));
    }

    let needs_pressed_fn = component == "Pressable" && !pressed_parts.is_empty();
    if needs_pressed_fn {
        style_array_parts.extend(pressed_parts);
    } else if !pressed_parts.is_empty() {
        // `pressed` comes from Pressable's render-prop `style` form, which
        // only Pressable has. On a View or Text a function isn't a valid
        // `style` value at all, so there's nowhere for these to go.
        diagnostics.push(unwired_variant(
            node,
            &format!(
                "`pressed:` needs an element that tracks press state, and `{component}` doesn't. \
                 Move it to a Pressable or Button."
            ),
            Severity::Error,
        ));
    }

    let mut props_text = String::new();
    if needs_pressed_fn {
        props_text.push_str(&format!(" style={{({{ pressed }}) => [{}]}}", style_array_parts.join(", ")));
    } else if style_array_parts.len() == 1 && !style_array_parts[0].contains("&&") {
        props_text.push_str(&format!(" style={{{}}}", style_array_parts[0]));
    } else if !style_array_parts.is_empty() {
        props_text.push_str(&format!(" style={{[{}]}}", style_array_parts.join(", ")));
    }
    for (key, value) in &extra_props {
        props_text.push_str(&format!(r#" {key}="{value}""#));
    }
    // Styles that RN expresses as props (see `truncation_props`).
    // `numberOfLines` takes a number, so it's braced rather than quoted.
    for (key, value) in truncation.into_iter().flatten() {
        if value.parse::<u32>().is_ok() {
            props_text.push_str(&format!(" {key}={{{value}}}"));
        } else {
            props_text.push_str(&format!(r#" {key}="{value}""#));
        }
    }
    if let Some(on_press) = node.props.on_press {
        props_text.push_str(&format!(" onPress={{{}}}", source_text(source, on_press)));
    }
    if let Some(disabled) = &node.props.disabled {
        props_text.push_str(&format!(" disabled={{{}}}", render_condition_expr(source, disabled)));
    }
    // Everything Dowel doesn't model, re-emitted verbatim and last so JSX's
    // last-wins duplicate resolution keeps matching the source's own
    // ordering semantics.
    for prop in &node.props.passthrough {
        props_text.push(' ');
        props_text.push_str(source_text(source, prop.span));
    }

    let inner = match &node.text {
        Some(TextContent::Literal(text)) => {
            let escaped = escape_jsx_text(text);
            if wraps_text {
                wrap_in_text(
                    &escaped,
                    &text_declarations,
                    &base_name,
                    source,
                    node,
                    position,
                    style_entries,
                    diagnostics,
                )
            } else {
                escaped
            }
        }
        Some(TextContent::Dynamic(_)) | None => node
            .children
            .iter()
            .enumerate()
            .map(|(index, child)| {
                // A child's position is only trustworthy when nothing was
                // dropped from `children` -- an unmodeled component or an
                // expression container renders and takes a slot without
                // becoming a `Node`.
                let child_position = if !node.children_complete {
                    SiblingPosition::Unknown
                } else if index == 0 {
                    SiblingPosition::First
                } else {
                    SiblingPosition::NotFirst
                };
                render_node(child, child_position, source, allocator, style_entries, diagnostics)
            })
            .collect(),
    };

    format!("<{component}{props_text}>{inner}</{component}>")
}

/// Builds the inserted `<Text>` that carries a non-Text node's string
/// content, with the text-styling declarations moved onto it.
fn wrap_in_text(
    content: &str,
    text_declarations: &[StyleDeclaration],
    base_name: &str,
    source: &str,
    node: &Node,
    // The *enclosing* node's position, not the wrapper's: these
    // declarations were written on that element, so a `first:` among them
    // asks about it. (The wrapper is trivially its parent's only child,
    // which is not the question.)
    position: SiblingPosition,
    style_entries: &mut Vec<(String, Vec<StyleProperty>)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> String {
    let mut style_array_parts = Vec::new();
    // The wrapper is a Text, so a `pressed:` style has nowhere to go on it.
    // The enclosing Pressable reports it -- `build_style_entries` runs over
    // that node's own declarations first, and a text-styling property under
    // `pressed:` lands here only after that.
    let mut pressed_parts = Vec::new();
    build_style_entries(
        text_declarations,
        &format!("{base_name}_text"),
        source,
        node,
        position,
        style_entries,
        &mut style_array_parts,
        &mut pressed_parts,
        diagnostics,
    );

    let style_prop = if style_array_parts.is_empty() {
        String::new()
    } else if style_array_parts.len() == 1 && !style_array_parts[0].contains("&&") {
        format!(" style={{{}}}", style_array_parts[0])
    } else {
        format!(" style={{[{}]}}", style_array_parts.join(", "))
    };
    format!("<Text{style_prop}>{content}</Text>")
}

/// Properties that style text itself. They matter separately on this
/// platform because React Native's `Text` inherits them from an enclosing
/// `Text` but not from a `View`, so they have to travel with the text
/// rather than stay on its container.
fn is_text_property(property: &StyleProperty) -> bool {
    matches!(
        property,
        StyleProperty::FontSize(_)
            | StyleProperty::FontWeight(_)
            | StyleProperty::LineHeight(_)
            | StyleProperty::LetterSpacing(_)
            | StyleProperty::TextColor(_)
            | StyleProperty::TextAlign(_)
            | StyleProperty::TextTransform(_)
    )
}

fn unwired_variant(node: &Node, message: &str, severity: Severity) -> Diagnostic {
    Diagnostic {
        code: DiagnosticCode::VariantNotWiredOnNative,
        severity,
        message: message.to_string(),
        span: node.span,
    }
}

/// Groups `declarations` by condition, registers a named style entry for
/// each group, and records how each should be referenced from the rendered
/// `style` prop. Shared by a node and any `Text` wrapper inserted inside
/// it, so both get identical condition handling.
///
/// Every condition that can't reach the rendered `style` prop reports
/// itself. Until 2026-08-15 they were computed into the StyleSheet and then
/// dropped in silence -- all eight variant-prefixed utilities in the
/// conformance suite, scored as covered because the entry existed.
#[allow(clippy::too_many_arguments)]
fn build_style_entries(
    declarations: &[StyleDeclaration],
    base_name: &str,
    source: &str,
    node: &Node,
    position: SiblingPosition,
    style_entries: &mut Vec<(String, Vec<StyleProperty>)>,
    style_array_parts: &mut Vec<String>,
    pressed_parts: &mut Vec<String>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // A conditional style must land after every unconditional one,
    // whatever order they were written in. On Web the cascade settles this
    // by specificity -- `.dowel-0:disabled` (0,2,0) beats `.dowel-0`
    // (0,1,0) no matter which rule comes first -- but a React Native style
    // array resolves purely last-wins, so position has to stand in for
    // specificity. Writing `disabled:p-8 p-4` used to render p-8 on Web and
    // p-4 on device.
    //
    // Within each half, source order is preserved: two conditions are the
    // same specificity on Web, so there it is source order that decides.
    let mut base_parts: Vec<String> = Vec::new();
    let mut conditional_parts: Vec<String> = Vec::new();

    for (condition, props) in dowel_ir::group_by_condition(declarations) {
        let props = dowel_ir::dedupe_last_wins(props);
        if props.is_empty() {
            continue;
        }
        let name = match condition_suffix(&condition) {
            None => base_name.to_string(),
            Some(suffix) => format!("{base_name}_{suffix}"),
        };
        match &condition {
            Condition::Always => base_parts.push(format!("styles.{name}")),
            Condition::Disabled => {
                if let Some(disabled) = &node.props.disabled {
                    let guard = render_condition_expr(source, disabled);
                    conditional_parts.push(format!("({guard}) && styles.{name}"));
                } else {
                    // Nothing on this element drives the condition. On Web
                    // the same source is inert too (`:disabled` never
                    // matches a div), but there it's CSS behaving
                    // correctly; here it's a style that was computed and
                    // then had nowhere to go.
                    diagnostics.push(unwired_variant(
                        node,
                        "`disabled:` needs a `disabled` prop on the same element to drive it, and \
                         this one has none.",
                        Severity::Error,
                    ));
                }
            }
            Condition::Pressed => pressed_parts.push(format!("pressed && styles.{name}")),
            Condition::Expr(expr) => {
                let guard = render_condition_expr(source, expr);
                conditional_parts.push(format!("({guard}) && styles.{name}"));
            }
            // Each of these produced a style object that the rendered JSX
            // never referenced -- computed, then dropped, with nothing
            // said. That silence is the bug being fixed here; the styles
            // still don't apply, but no longer without saying so.
            Condition::Hover => diagnostics.push(unwired_variant(
                node,
                "`hover:` isn't wired on React Native yet. It is a real condition there -- a \
                 tablet with a trackpad or pencil, and the macOS/Windows/visionOS targets, all \
                 report hover -- so this is unbuilt rather than impossible.",
                Severity::Warning,
            )),
            Condition::Focus => diagnostics.push(unwired_variant(
                node,
                "`focus:` isn't wired on React Native yet.",
                Severity::Warning,
            )),
            Condition::Responsive(_) => diagnostics.push(unwired_variant(
                node,
                "Breakpoint variants (`sm:`/`md:`/`lg:`/`xl:`/`2xl:`) aren't wired on React \
                 Native yet, so this style never applies -- a tablet or landscape layout that \
                 depends on it will be wrong.",
                Severity::Error,
            )),
            Condition::Dark => diagnostics.push(unwired_variant(
                node,
                "`dark:` isn't wired on React Native yet, so this style never applies and the \
                 element keeps its light-mode appearance in dark mode.",
                Severity::Error,
            )),
            // Resolved at build time rather than needing a selector
            // engine. Both decided answers are exact -- the same thing
            // `:first-child` would do on Web -- so neither reports
            // anything; only an undecidable position does.
            Condition::FirstChild => match position {
                SiblingPosition::First => conditional_parts.push(format!("styles.{name}")),
                // `:first-child` wouldn't match here either, so dropping
                // the style is the correct outcome, not a gap.
                SiblingPosition::NotFirst => {}
                SiblingPosition::Unknown => diagnostics.push(unwired_variant(
                    node,
                    "`first:` can only be resolved when the compiler can see this element's \
                     position among its siblings, and here it can't -- it's either the root of a \
                     component (whose position its caller decides) or a sibling of something \
                     Dowel doesn't model, such as a custom component or a `{...}` expression.",
                    Severity::Error,
                )),
            },
        }
        // No catch-all arm above, deliberately: a new `Condition` variant
        // must fail to compile here rather than quietly joining the set
        // that gets computed and dropped. That is exactly how the eight
        // variants this function now reports went unnoticed.
        style_entries.push((name, props));
    }

    style_array_parts.append(&mut base_parts);
    style_array_parts.append(&mut conditional_parts);
    // `pressed_parts` is appended by the caller, after these, because only
    // there is it known whether the element can carry press state at all.
    // That puts `pressed:` last among the conditions rather than in source
    // order relative to them -- a divergence from Web only when a
    // `pressed:` utility and another conditional set the same property.
}

/// React Native expresses text truncation as props on `Text` --
/// `numberOfLines` and `ellipsizeMode` -- where CSS uses `white-space` and
/// `text-overflow`. The mapping is from the *combination* of declarations
/// to one prop pair, not property-by-property, which is why it lives here
/// rather than in `style::property_and_value`.
///
/// `None` means this node can't absorb them (nothing asked for truncation,
/// or it isn't a `Text`), and the caller refuses them instead.
fn truncation_props(node: &Node) -> Option<Vec<(&'static str, String)>> {
    // `numberOfLines` exists on Text alone; on a View there's nothing to
    // put it on, so truncation there really is unsupported.
    if node.primitive != Primitive::Text {
        return None;
    }
    let has = |want: &StyleProperty| node.style.iter().any(|d| d.property == *want);
    if !has(&StyleProperty::WhiteSpace(WhiteSpace::NoWrap)) {
        return None;
    }

    let mut props = vec![("numberOfLines", "1".to_string())];
    if !has(&StyleProperty::TextOverflow(TextOverflow::Ellipsis)) {
        // RN's default `ellipsizeMode` is `tail`, i.e. an ellipsis. Nothing
        // asked for one here, so clipping is the closer match to plain
        // `white-space: nowrap`.
        props.push(("ellipsizeMode", "clip".to_string()));
    }
    Some(props)
}

fn is_truncation_declaration(property: &StyleProperty) -> bool {
    matches!(
        property,
        StyleProperty::WhiteSpace(WhiteSpace::NoWrap) | StyleProperty::TextOverflow(_)
    )
}

/// Why a truncation-related declaration can't be honoured when it wasn't
/// absorbed into props. Kept out of `StyleProperty::unsupported_on_native`
/// because the answer depends on the node, which that method can't see.
fn truncation_only_reason(property: &StyleProperty) -> Option<String> {
    match property {
        StyleProperty::TextOverflow(_) => Some(
            "`text-overflow`: React Native truncates via the `numberOfLines` prop on Text, which \
             needs `white-space: nowrap` (Tailwind's `truncate`) on a Text element."
                .to_string(),
        ),
        StyleProperty::WhiteSpace(WhiteSpace::NoWrap) => Some(
            "`white-space: nowrap`: React Native suppresses wrapping with the `numberOfLines` \
             prop, which only exists on Text."
                .to_string(),
        ),
        _ => None,
    }
}

fn escape_jsx_text(text: &str) -> String {
    text.replace('{', "&#123;").replace('}', "&#125;")
}

/// `None` for `Always` (uses the node's base style name directly);
/// otherwise a name-safe suffix identifying the condition.
fn condition_suffix(condition: &Condition) -> Option<String> {
    match condition {
        Condition::Always => None,
        Condition::Hover => Some("hover".to_string()),
        Condition::Focus => Some("focus".to_string()),
        Condition::Disabled => Some("disabled".to_string()),
        Condition::Pressed => Some("pressed".to_string()),
        Condition::Dark => Some("dark".to_string()),
        Condition::FirstChild => Some("first".to_string()),
        Condition::Responsive(bp) => Some(
            match bp {
                Breakpoint::Sm => "sm",
                Breakpoint::Md => "md",
                Breakpoint::Lg => "lg",
                Breakpoint::Xl => "xl",
                Breakpoint::Xl2 => "xl2",
            }
            .to_string(),
        ),
        Condition::Expr(expr) => {
            let mut refs = Vec::new();
            collect_expr_refs(expr, &mut refs);
            Some(format!(
                "cond_{}",
                refs.iter().map(|r: &ExprRef| format!("{}_{}", r.0.start, r.0.end)).collect::<Vec<_>>().join("_")
            ))
        }
    }
}

fn collect_expr_refs(expr: &ConditionExpr, out: &mut Vec<ExprRef>) {
    match expr {
        ConditionExpr::Ref(r) => out.push(*r),
        ConditionExpr::Not(inner) => collect_expr_refs(inner, out),
        ConditionExpr::And(a, b) | ConditionExpr::Or(a, b) => {
            collect_expr_refs(a, out);
            collect_expr_refs(b, out);
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
    fn lowers_the_login_example_to_rn_jsx_and_styles() {
        let parsed = dowel_parser::parse_tsx(LOGIN_EXAMPLE);
        let root = &parsed.roots[0];
        let output = lower(root, LOGIN_EXAMPLE);

        assert!(output.jsx.starts_with("<View style={styles.dowel0}>"));
        assert!(output.jsx.contains("<Text style={styles.dowel1}>Welcome</Text>"));
        // The label is wrapped: React Native crashes on a raw string inside
        // a Pressable, even though the same source is fine on Web.
        assert!(output.jsx.contains(
            r#"<Pressable style={styles.dowel2} accessibilityRole="button"><Text>Continue</Text></Pressable>"#
        ));

        assert!(output.styles.contains("dowel0: {"));
        assert!(output.styles.contains("flex: 1,"));
        assert!(output.styles.contains("paddingTop: 24,"));
        assert!(output.styles.contains("dowel1: {"));
        assert!(output.styles.contains("fontSize: 20,"));
        assert!(output.styles.contains("fontWeight: '700',"));
        assert!(output.styles.contains("dowel2: {"));
        // `px-4` is Tailwind's logical inline axis, so this lowers to RN's
        // direction-relative props rather than paddingLeft/paddingRight.
        assert!(output.styles.contains("paddingStart: 16,"));
        assert!(output.styles.contains("paddingEnd: 16,"));
        // No `px`/CSS units anywhere -- these are unitless RN numbers.
        assert!(!output.styles.contains("px"));

        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn disabled_condition_merges_into_a_conditional_style_array_when_a_disabled_prop_exists() {
        let source = r#"
            import { Button } from '@dowel/core'
            const el = <Button disabled={isLoading} className="p-2 disabled:opacity-50">Save</Button>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.styles.contains("dowel0_disabled: {"));
        assert!(output.styles.contains("opacity: 0.5,"));
        assert!(output.jsx.contains("style={[styles.dowel0, (isLoading) && styles.dowel0_disabled]}"));
        assert!(output.jsx.contains("disabled={isLoading}"));
    }

    #[test]
    fn unmodeled_props_and_spreads_reach_the_output() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-4" {...rest} onLayout={onLayout} testID="row" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.jsx.contains("{...rest}"));
        assert!(output.jsx.contains("onLayout={onLayout}"));
        assert!(output.jsx.contains(r#"testID="row""#));
    }

    #[test]
    fn pressed_condition_wraps_style_in_rn_pressable_render_prop() {
        let source = r#"
            import { Button } from '@dowel/core'
            const el = <Button className="p-2 pressed:opacity-50">Save</Button>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.styles.contains("dowel0_pressed: {"));
        assert!(output.styles.contains("opacity: 0.5,"));
        assert!(output.jsx.contains("style={({ pressed }) => [styles.dowel0, pressed && styles.dowel0_pressed]}"));
    }

    #[test]
    fn pressed_condition_stays_unmerged_on_view_since_style_cannot_be_a_function_there() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-2 pressed:opacity-50" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.styles.contains("dowel0_pressed: {"));
        assert!(output.jsx.contains("style={styles.dowel0}"));
        assert!(!output.jsx.contains("pressed"));
    }

    #[test]
    fn disabled_condition_stays_unmerged_without_a_disabled_prop() {
        // Nothing drives "disabled-ness" here -- the className has a
        // disabled: variant but the component never actually received a
        // `disabled` prop, so there's no guard to merge with. Computed,
        // not silently dropped, but also not merged into anything.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="disabled:opacity-50" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.styles.contains("dowel0_disabled: {"));
        assert!(!output.jsx.contains("dowel0_disabled"));
    }

    #[test]
    fn dynamic_class_name_guard_merges_into_the_style_array() {
        let source = r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', active && 'text-xl')} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.jsx.contains("style={[styles.dowel0, (active) && styles.dowel0_cond_"));
    }

    #[test]
    fn hover_and_focus_still_do_not_merge_into_anything() {
        // No RN mechanism for either (see module docs) -- still computed,
        // still not merged, unlike Disabled/Expr which now are.
        let node = dowel_ir::Node {
            primitive: dowel_ir::Primitive::View,
            style: vec![
                dowel_ir::StyleDeclaration {
                    property: dowel_ir::StyleProperty::Opacity(1.0),
                    condition: dowel_ir::Condition::Always,
                },
                dowel_ir::StyleDeclaration {
                    property: dowel_ir::StyleProperty::Opacity(0.5),
                    condition: dowel_ir::Condition::Hover,
                },
            ],
            props: dowel_ir::PropSet::default(),
            children: Vec::new(),
            text: None,
            class_name_fallback: Vec::new(),
            children_complete: true,
            span: dowel_ir::SourceSpan { start: 0, end: 0 },
        };
        let output = lower(&node, "");
        assert!(output.jsx.contains("style={styles.dowel0}"));
        assert!(output.styles.contains("dowel0_hover: {"));
        assert!(!output.jsx.contains("dowel0_hover"));
    }

    #[test]
    fn transforms_compose_into_rn_single_transform_array() {
        // RN has no standalone rotate/scale/translate, so several IR
        // properties collapse into one entry -- ordered translate, rotate,
        // scale to match how CSS applies its standalone equivalents.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="scale-95 rotate-45 translate-x-2" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.styles.contains(
            "transform: [{ translateX: 8 }, { rotate: '45deg' }, { scale: 0.95 }],"
        ));
    }

    #[test]
    fn shadow_and_filter_carry_across_as_strings() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="shadow-lg blur-sm" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.styles.contains("boxShadow: '0 10px 15px -3px"));
        assert!(output.styles.contains("filter: 'blur(8px)',"));
    }

    #[test]
    fn web_only_display_is_refused_rather_than_dropped() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="block" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::WebOnlyPropertyOnNative);
        assert_eq!(output.diagnostics[0].severity, dowel_ir::Severity::Error);
        // And nothing is emitted for it, so a build that ignored the error
        // still can't produce an invalid RN style value.
        assert!(!output.styles.contains("display"));
    }

    /// Every variant that can't reach the `style` prop, with the severity
    /// it should report. Until 2026-08-15 all of these produced a
    /// StyleSheet entry the JSX never referenced, and said nothing -- the
    /// conformance suite scored them covered because the entry existed.
    #[test]
    fn no_variant_is_dropped_without_saying_so() {
        let cases: &[(&str, dowel_ir::Severity)] = &[
            // Real on tablets with a pointer and on the desktop targets --
            // unbuilt, not impossible, so it warns rather than stopping a
            // cross-platform build.
            ("hover:bg-blue-500", dowel_ir::Severity::Warning),
            ("focus:p-4", dowel_ir::Severity::Warning),
            // These have plain React Native counterparts and dropping them
            // renders the wrong thing, so they stop the build.
            ("md:p-4", dowel_ir::Severity::Error),
            ("dark:p-4", dowel_ir::Severity::Error),
            ("first:mt-0", dowel_ir::Severity::Error),
            // Nothing on a bare View drives these at all.
            ("disabled:p-4", dowel_ir::Severity::Error),
            ("pressed:p-4", dowel_ir::Severity::Error),
        ];

        for (candidate, severity) in cases {
            let source = format!(
                "import {{ View }} from '@dowel/core'\nconst el = <View className=\"{candidate}\" />\n"
            );
            let parsed = dowel_parser::parse_tsx(&source);
            let output = lower(&parsed.roots[0], &source);

            let reported: Vec<_> = output
                .diagnostics
                .iter()
                .filter(|d| d.code == dowel_ir::DiagnosticCode::VariantNotWiredOnNative)
                .collect();
            assert_eq!(reported.len(), 1, "{candidate}: {:?}", output.diagnostics);
            assert_eq!(reported[0].severity, *severity, "{candidate}");
        }
    }

    #[test]
    fn a_conditional_style_outranks_the_base_whatever_order_it_was_written_in() {
        // Web settles this by specificity: `.dowel-0:disabled` (0,2,0)
        // beats `.dowel-0` (0,1,0) regardless of which rule comes first. A
        // React Native style array only resolves last-wins, so position has
        // to stand in for specificity -- otherwise `disabled:p-8 p-4`
        // renders p-8 on Web and p-4 on device.
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = (
              <Pressable className="disabled:p-8 p-4" disabled={off}
                accessibilityRole="button">x</Pressable>
            )
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        let base = output.jsx.find("styles.dowel0,").expect("base style");
        let conditional = output.jsx.find("styles.dowel0_disabled").expect("conditional style");
        assert!(base < conditional, "{}", output.jsx);
    }

    #[test]
    fn first_child_is_decided_at_compile_time() {
        // Web asks `:first-child` at match time; here the compiler is
        // looking straight at the JSX tree and already knows. Both answers
        // are exact, so neither reports anything.
        let source = r#"
            import { View, Text } from '@dowel/core'
            const el = (
              <View>
                <Text className="first:mt-0">a</Text>
                <Text className="first:mt-0">b</Text>
              </View>
            )
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        // The first child gets it applied unconditionally...
        assert!(output.jsx.contains("styles.dowel1_first"), "{}", output.jsx);
        // ...and the second doesn't get one at all, which is exactly what
        // `:first-child` would do.
        assert!(!output.jsx.contains("styles.dowel2_first"), "{}", output.jsx);
    }

    #[test]
    fn first_child_is_refused_when_a_sibling_is_unmodeled() {
        // `<Avatar/>` renders and occupies the first slot, but never
        // becomes a Node -- so the Text is index 0 in `children` and second
        // on screen. Deciding from that index would apply the style to the
        // wrong element, silently.
        let source = r#"
            import { View, Text } from '@dowel/core'
            const el = (
              <View>
                <Avatar />
                <Text className="first:mt-0">b</Text>
              </View>
            )
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        let reported: Vec<_> = output
            .diagnostics
            .iter()
            .filter(|d| d.code == dowel_ir::DiagnosticCode::VariantNotWiredOnNative)
            .collect();
        assert_eq!(reported.len(), 1, "{:?}", output.diagnostics);
        assert!(reported[0].message.contains("position"), "{}", reported[0].message);
    }

    #[test]
    fn first_child_is_refused_on_a_component_root() {
        // Where this element sits is its caller's decision, not something
        // visible from here.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="first:mt-0" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output
            .diagnostics
            .iter()
            .any(|d| d.code == dowel_ir::DiagnosticCode::VariantNotWiredOnNative));
    }

    #[test]
    fn a_wired_variant_reports_nothing() {
        // The two that do work must not have been swept up in the above.
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = (
              <Pressable className="pressed:p-4 disabled:opacity-50" disabled={isOff}
                accessibilityRole="button">x</Pressable>
            )
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(
            output.diagnostics.is_empty(),
            "{:?}",
            output.diagnostics.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        assert!(output.jsx.contains("pressed && styles."), "{}", output.jsx);
        assert!(output.jsx.contains("(isOff) && styles."), "{}", output.jsx);
    }

    #[test]
    fn an_unresolvable_class_name_is_handed_to_the_runtime_resolver() {
        // Web concatenates it back on and lets the browser's CSS engine
        // match it. RN has neither a className nor a CSS engine, so the
        // expression goes to the generated resolver instead -- warned
        // about, since only unconditional classes survive that path.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className={classNameFromProps} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics[0].code,
            dowel_ir::DiagnosticCode::DynamicClassNameNotResolved
        );
        assert_eq!(output.diagnostics[0].severity, dowel_ir::Severity::Warning);
        assert!(output.jsx.contains("dowelClasses(classNameFromProps)"), "{}", output.jsx);
    }

    #[test]
    fn the_runtime_resolved_part_comes_last_so_it_wins() {
        // `cn('p-4', getDynamic())` puts the opaque part last in the
        // source, and RN merges a style array last-wins -- so the compiled
        // styles must not be able to override it.
        let source = r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', getDynamic())} />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        let compiled = output.jsx.find("styles.dowel0").expect("compiled styles");
        let dynamic = output.jsx.find("dowelClasses(").expect("resolver call");
        assert!(compiled < dynamic, "{}", output.jsx);
    }

    #[test]
    fn the_candidate_module_maps_class_names_to_style_objects() {
        let module = render_candidate_module(&["p-4".to_string(), "bg-blue-500".to_string()]);
        assert!(module.contains(r#""p-4": {"#), "{module}");
        assert!(module.contains("paddingTop: 16,"), "{module}");
        assert!(module.contains(r#""bg-blue-500": {"#), "{module}");
        assert!(module.contains("createClassResolver(styles, unsupported)"), "{module}");
    }

    #[test]
    fn conditional_candidates_are_named_rather_than_silently_missing() {
        // A style object can't carry `hover:`, and making it able to means
        // per-component state tracking -- the engine this design is
        // choosing not to ship. Reported when used, not at build time:
        // appearing in the scan doesn't prove anything produces it.
        let module = render_candidate_module(&["hover:bg-blue-500".to_string()]);
        assert!(!module.contains("styles = {\n  \"hover"), "{module}");
        assert!(module.contains(r#""hover:bg-blue-500": "`hover:bg-blue-500` is conditional"#), "{module}");
    }

    #[test]
    fn web_only_candidates_are_named_too() {
        let module = render_candidate_module(&["grid".to_string()]);
        assert!(module.contains(r#""grid": ""#), "{module}");
        assert!(module.contains("Web-only"), "{module}");
    }

    #[test]
    fn unrecognized_candidates_are_skipped_entirely() {
        // Scanning is imprecise by design; a token that only looked like a
        // class is neither a style nor a problem to report.
        let module = render_candidate_module(&["useState".to_string()]);
        assert!(!module.contains("useState"), "{module}");
    }

    #[test]
    fn raw_text_in_a_view_is_wrapped_and_takes_its_text_styles_with_it() {
        // Two separate hazards, both invisible on Web: a raw string inside
        // a View crashes React Native, and `fontSize` left on the View
        // would do nothing there because Text doesn't inherit from View.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-4 text-xl font-bold">Hello</View>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert!(output.jsx.contains("<Text style={styles.dowel0_text}>Hello</Text>"));
        // Layout stays on the View, text styling moves to the Text.
        assert!(output.styles.contains("paddingTop: 16,"));
        assert!(output.styles.contains("dowel0_text: {"));
        assert!(output.styles.contains("fontSize: 20,"));
        assert!(output.styles.contains("fontWeight: '700',"));
        // Not left behind on the container, where RN would ignore it.
        let container = output.styles.split("dowel0_text").next().unwrap();
        assert!(!container.contains("fontSize"));
    }

    #[test]
    fn a_text_node_is_not_double_wrapped() {
        let source = r#"
            import { Text } from '@dowel/core'
            const el = <Text className="text-xl">Hello</Text>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert_eq!(output.jsx.matches("<Text").count(), 1);
        assert!(output.styles.contains("fontSize: 20,"));
    }

    #[test]
    fn truncation_lowers_to_props_rather_than_styles() {
        // RN has no white-space/text-overflow; it truncates via props.
        // `truncate` asks for an ellipsis, which is `ellipsizeMode`'s
        // default, so only `numberOfLines` is needed.
        let source = r#"
            import { Text } from '@dowel/core'
            const el = <Text className="truncate">x</Text>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.diagnostics.is_empty());
        assert!(output.jsx.contains("numberOfLines={1}"));
        assert!(!output.jsx.contains("ellipsizeMode"));
        // The `overflow` half of `truncate` is a real RN style and still
        // lowers as one.
        assert!(output.styles.contains("overflow: 'hidden',"));
    }

    #[test]
    fn nowrap_without_ellipsis_clips_instead() {
        let source = r#"
            import { Text } from '@dowel/core'
            const el = <Text className="whitespace-nowrap">x</Text>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.diagnostics.is_empty());
        assert!(output.jsx.contains("numberOfLines={1}"));
        // Nothing asked for an ellipsis, and RN's default would add one.
        assert!(output.jsx.contains(r#"ellipsizeMode="clip""#));
    }

    #[test]
    fn truncation_on_a_non_text_node_is_refused() {
        // `numberOfLines` only exists on Text, so there's nothing to
        // absorb it into here -- and silently dropping it would lose the
        // author's intent.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="truncate" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(!output.diagnostics.is_empty());
        assert_eq!(output.diagnostics[0].severity, dowel_ir::Severity::Error);
    }

    #[test]
    fn whitespace_normal_stays_a_genuine_no_op() {
        // RN's Text already wraps, so this asks for what happens anyway.
        let source = r#"
            import { Text } from '@dowel/core'
            const el = <Text className="whitespace-normal">x</Text>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.diagnostics.is_empty());
        assert!(!output.jsx.contains("numberOfLines"));
    }

    #[test]
    fn viewport_height_is_refused_and_leaves_valid_output() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="h-screen" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);

        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::WebOnlyPropertyOnNative);
        assert_eq!(output.diagnostics[0].severity, dowel_ir::Severity::Error);
        // The key must be dropped entirely, not written with an empty
        // value -- `height: ,` isn't parseable JS.
        assert!(!output.styles.contains("height"));
        assert!(!output.styles.contains(": ,"));
    }

    #[test]
    fn portable_display_values_lower_normally() {
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="hidden" />
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert!(output.diagnostics.is_empty());
        assert!(output.styles.contains("display: 'none',"));
    }

    #[test]
    fn interactive_pressable_without_role_is_diagnosed_from_real_source() {
        // As with dowel_web: previously only reachable by hand-constructing
        // a `Node` -- the parser didn't populate on_press/accessibility_role
        // at all until dowel_parser::jsx gained that attribute parsing.
        let source = r#"
            import { Pressable } from '@dowel/core'
            const el = <Pressable onPress={handleTap}>Tap</Pressable>
            "#;
        let parsed = dowel_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0], source);
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, dowel_ir::DiagnosticCode::A11yInteractiveWithoutRole);
        assert!(output.jsx.contains("onPress={handleTap}"));

        let source_with_role = r#"
            import { Pressable } from '@dowel/core'
            const el = (
              <Pressable onPress={handleTap} accessibilityRole="button">Tap</Pressable>
            )
            "#;
        let parsed_with_role = dowel_parser::parse_tsx(source_with_role);
        let output_with_role = lower(&parsed_with_role.roots[0], source_with_role);
        assert!(output_with_role.diagnostics.is_empty());
        assert!(output_with_role.jsx.contains(r#"accessibilityRole="button""#));
    }
}
