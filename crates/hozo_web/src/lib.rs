//! Hozo IR to DOM/CSS/ARIA lowering (Web backend).
//!
//! Emits per-node scoped CSS classes rather than deduplicating/reusing
//! atomic utility classes across call sites -- a constraint carried over
//! from the cascade-ordering design discussion, since it's what lets
//! simple source-order-within-condition flattening (see `css.rs`) stay
//! correct without needing RNW/StyleX-style explicit priority tables.

mod css;
mod markup;

use hozo_ir::{Diagnostic, Node, Primitive, Theme};

pub struct LowerOutput {
    pub jsx: String,
    pub css: String,
    /// Named imports `jsx` needs from `@hozo/runtime`.
    ///
    /// The Web backend's first, and it exists for one reason: a
    /// `<div role="button">` that Hozo put in the tab order has to be
    /// activatable from the keyboard, and only script can do that. The
    /// Native backend has carried the same field since it needed hooks.
    pub runtime_imports: Vec<&'static str>,
    pub diagnostics: Vec<Diagnostic>,
}

/// What a synthesized interactive element imports from `@hozo/runtime`.
///
/// Two module-level functions rather than one inline arrow per element:
/// they close over nothing (`event.currentTarget.click()` is how they reach
/// the author's `onClick`, which also means the author's handler still
/// receives a real MouseEvent rather than a keyboard one it isn't typed
/// for), so a stable reference costs nothing and allocates nothing.
const KEY_ACTIVATION_IMPORTS: &[&str] = &["hozoInteractive"];

/// `accessibilityState`'s keys, and the ARIA attribute each becomes.
///
/// The names come from React Native's `AccessibilityState`; the mapping is
/// one to one. Which of these a given `role` actually permits is a
/// separate question -- `role="button"` allows only `aria-disabled`,
/// `aria-busy` and `aria-expanded` -- and belongs with the ARIA validity
/// diagnostics rather than here, where the author's stated intent is
/// carried through.
const ARIA_STATE_ATTRS: &[(&str, &str)] = &[
    ("aria-disabled", "disabled"),
    ("aria-selected", "selected"),
    ("aria-checked", "checked"),
    ("aria-busy", "busy"),
    ("aria-expanded", "expanded"),
];

/// The proposal §8.1 "hozo-view" shared base style: applied to every
/// `View`, emitted once as a shared rule rather than duplicated per node.
const VIEW_BASE_CSS: &str = ".hozo-view {\n  \
    display: flex;\n  \
    flex-direction: column;\n  \
    flex-shrink: 0;\n  \
    position: relative;\n  \
    min-width: 0;\n  \
    box-sizing: border-box;\n\
}\n\n";

const SCROLL_VIEW_BASE_CSS: &str = ".hozo-scroll-view {\n  \
    overflow-x: hidden;\n  \
    overflow-y: auto;\n  \
    -webkit-overflow-scrolling: touch;\n\
}\n\n\
.hozo-scroll-view[data-hozo-horizontal] {\n  \
    overflow-x: auto;\n  \
    overflow-y: hidden;\n\
}\n\n\
.hozo-scroll-view[data-hozo-hide-scrollbar] {\n  \
    scrollbar-width: none;\n\
}\n\n\
.hozo-scroll-view[data-hozo-hide-scrollbar]::-webkit-scrollbar {\n  \
    display: none;\n\
}\n\n";

const POINTER_EVENTS_BASE_CSS: &str = "[data-hozo-pointer-events='none'] { pointer-events: none; }\n\
[data-hozo-pointer-events='auto'] { pointer-events: auto; }\n\
[data-hozo-pointer-events='box-none'] { pointer-events: none; }\n\
[data-hozo-pointer-events='box-none'] > * { pointer-events: auto; }\n\
[data-hozo-pointer-events='box-only'] { pointer-events: auto; }\n\
[data-hozo-pointer-events='box-only'] > * { pointer-events: none; }\n\n";

struct ClassAllocator {
    next: u32,
}

impl ClassAllocator {
    fn alloc(&mut self) -> String {
        let name = format!("hozo-{}", self.next);
        self.next += 1;
        name
    }
}

/// `source` is the original TSX text `root` was parsed from -- needed to
/// re-emit `ExprRef`/`ConditionExpr` guards verbatim (they're spans into
/// it, not evaluated by the compiler; see `hozo_ir`'s doc comments).
///
/// CSS for classes that can only be known at runtime is *not* emitted
/// here: candidates are a project-wide set, so their stylesheet is built
/// once by `render_candidate_stylesheet` rather than per file.
pub fn lower(root: &Node, source: &str, theme: &Theme) -> LowerOutput {
    let mut allocator = ClassAllocator { next: 0 };
    let mut rules = String::new();
    let mut diagnostics = Vec::new();
    let mut uses_view_base = false;
    let mut uses_key_activation = false;

    let jsx = render_node(
        root,
        source,
        theme,
        &mut allocator,
        &mut rules,
        &mut diagnostics,
        &mut uses_view_base,
        &mut uses_key_activation,
    );

    let mut css = String::new();
    if uses_view_base {
        css.push_str(VIEW_BASE_CSS);
    }
    if contains_primitive(root, Primitive::ScrollView) {
        css.push_str(SCROLL_VIEW_BASE_CSS);
    }
    if contains_prop(root, |node| node.props.pointer_events.is_some()) {
        css.push_str(POINTER_EVENTS_BASE_CSS);
    }
    // An `animation` declaration is inert without its `@keyframes`, and
    // those are document-level rather than per-node -- so they're collected
    // across the whole tree and emitted once, deduplicated.
    for keyframes in collect_keyframes(root) {
        css.push_str(keyframes);
        css.push_str("\n\n");
    }
    css.push_str(&rules);

    let runtime_imports = if uses_key_activation {
        KEY_ACTIVATION_IMPORTS.to_vec()
    } else {
        Vec::new()
    };
    LowerOutput { jsx, css, runtime_imports, diagnostics }
}

/// One stylesheet for every candidate class in the project, written under
/// the classes' real Tailwind names.
///
/// This is what makes proposal §7's third tier do something: a `className`
/// the compiler couldn't read is passed through, evaluates to a class
/// string at runtime, and finds a matching rule here. No runtime code is
/// involved -- the browser's CSS engine does the resolution.
///
/// Project-wide rather than per-file because the union is: a class written
/// in one module can be produced by an expression in another. Emitting it
/// per file would ship the whole set once per file that needs any of it.
///
/// Unrecognized names are skipped rather than reported. The candidate list
/// comes from scanning, so it's expected to contain tokens that only
/// looked like classes.
pub fn render_candidate_stylesheet(class_names: &[String], theme: &Theme) -> String {
    let mut out = String::new();
    for name in class_names {
        let Some(utility) = hozo_parser::resolve_class_name(name) else {
            continue;
        };
        let selector = css::escape_class_selector(&utility.class_name);
        // One rule per group: a `container` is a width plus a max-width at
        // each breakpoint, which cannot be one rule.
        for (condition, properties) in &utility.groups {
            out.push_str(&css::render_rule(&selector, condition, properties, theme));
            out.push_str("\n\n");
        }
    }
    out
}

/// Every distinct `@keyframes` block the tree's animations need, in
/// first-use order so output stays deterministic.
fn collect_keyframes(node: &Node) -> Vec<&'static str> {
    let mut found: Vec<&'static str> = Vec::new();
    collect_keyframes_into(node, &mut found);
    found
}

fn contains_primitive(node: &Node, primitive: Primitive) -> bool {
    if node.primitive == primitive {
        return true;
    }
    node.children.iter().any(|child| match child {
        hozo_ir::Child::Node(child) => contains_primitive(child, primitive),
        hozo_ir::Child::Verbatim { nested, .. } => nested.iter().any(|entry| contains_primitive(&entry.node, primitive)),
        hozo_ir::Child::Text(_) => false,
    })
}

fn contains_prop(node: &Node, predicate: fn(&Node) -> bool) -> bool {
    if predicate(node) {
        return true;
    }
    node.children.iter().any(|child| match child {
        hozo_ir::Child::Node(child) => contains_prop(child, predicate),
        hozo_ir::Child::Verbatim { nested, .. } => nested.iter().any(|entry| contains_prop(&entry.node, predicate)),
        hozo_ir::Child::Text(_) => false,
    })
}

fn collect_keyframes_into(node: &Node, found: &mut Vec<&'static str>) {
    for declaration in &node.style {
        if let hozo_ir::StyleProperty::Animation(animation) = declaration.property {
            if let Some(keyframes) = animation.keyframes() {
                if !found.contains(&keyframes) {
                    found.push(keyframes);
                }
            }
        }
    }
    for child in &node.children {
        if let hozo_ir::Child::Node(child_node) = child {
            collect_keyframes_into(child_node, found);
        }
    }
}

/// Byte-slices `source` at an `ExprRef`'s span. Spans come from oxc's own
/// tokenizer over this same `source`, so they're always on UTF-8 character
/// boundaries -- not re-validated here.
fn source_text(source: &str, expr_ref: hozo_ir::ExprRef) -> &str {
    &source[expr_ref.0.start as usize..expr_ref.0.end as usize]
}

/// Re-emits a `ConditionExpr` as a JS boolean expression by splicing the
/// original source at each leaf `Ref`'s span -- the compiler never
/// evaluates these, only reconstructs them with real `&&`/`||`/`!`
/// wrapping the *combinator structure* it built (see hozo_parser's
/// `dynamic_class` module), not anything it parsed out of the leaves
/// themselves.
fn render_condition_expr(source: &str, expr: &hozo_ir::ConditionExpr) -> String {
    use hozo_ir::ConditionExpr;
    match expr {
        ConditionExpr::Static(value) => value.to_string(),
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
    theme: &Theme,
    allocator: &mut ClassAllocator,
    rules: &mut String,
    diagnostics: &mut Vec<Diagnostic>,
    uses_view_base: &mut bool,
    uses_key_activation: &mut bool,
) -> String {
    let class_name = allocator.alloc();

    // `rules` accumulates the whole tree's CSS, so "did this node write
    // one" has to be asked about the span this node adds, not about
    // whether the string is empty.
    let rules_before = rules.len();

    for (condition, props) in hozo_ir::group_by_condition(&node.style) {
        let props = hozo_ir::dedupe_last_wins(props);
        if props.is_empty() {
            continue;
        }
        rules.push_str(&css::render_rule(&class_name, &condition, &props, theme));
        rules.push_str("\n\n");
    }

    let (mut tag, extra_attrs) = markup::element_shape(node, diagnostics);
    let has_pan_handlers_spread = node.props.passthrough.iter().any(|prop| {
        prop.is_spread && source_text(source, prop.span).contains(".panHandlers")
    });
    if has_pan_handlers_spread {
        tag = match node.primitive {
            Primitive::View => "View",
            Primitive::Pressable => "Pressable",
            _ => tag,
        };
    }
    if node.primitive == Primitive::Image {
        if let Some(src) = node.props.image_src {
            let value = source_text(source, src);
            if !value.starts_with(['\"', '\'']) {
                tag = "Image";
            }
        }
    }
    // An element Hozo made into a control itself: a `<div>` it put in the
    // tab order, rather than a `<button>` the browser already knows about.
    // Everything a control needs then has to be supplied too, and has to
    // agree -- which is what `hozoInteractive` is for.
    let synthesized_control =
        node.props.on_press.is_some() && extra_attrs.iter().any(|(key, _)| *key == "tabIndex");

    // The one expression that decides `disabled`, whichever way it was
    // spelled. React Native folds them together -- `aria-disabled` and
    // `accessibilityState.disabled` are one state, and `Pressable.js`
    // merges the `disabled` prop into it -- so Hozo does too, rather than
    // emitting `aria-disabled` twice and letting the later one win by
    // accident, which is what writing both used to do.
    let state_disabled = node
        .props
        .accessibility_state
        .filter(|_| {
            node.props
                .accessibility_state_keys
                .as_ref()
                .is_none_or(|keys| keys.iter().any(|key| key == "disabled"))
        })
        .map(|value| format!("({}).disabled", source_text(source, value)));
    let disabled_expr = match (node.props.disabled.as_ref(), state_disabled) {
        (Some(flag), Some(state)) => {
            Some(format!("({}) || {state}", render_condition_expr(source, flag)))
        }
        (Some(flag), None) => Some(render_condition_expr(source, flag)),
        (None, state) => state,
    };

    let is_hozo_component = tag.starts_with("Hozo")
        || matches!(tag, "View" | "Text" | "Paragraph" | "Heading" | "Section" | "Article" | "Nav" | "List" | "ListItem" | "Image" | "ScrollView" | "FlatList" | "Pressable");

    // The generated class is dropped when no rule was written for it. It
    // matched nothing, so it was a class attribute on every unstyled
    // element and bytes in every render -- found by rendering the output
    // and comparing the classes in the DOM against the ones the stylesheet
    // defines, which is a comparison nothing had made before.
    let mut classes = if rules.len() == rules_before { String::new() } else { class_name };
    if node.primitive == Primitive::View {
        *uses_view_base = true;
        classes = if classes.is_empty() {
            "hozo-view".to_string()
        } else {
            format!("hozo-view {classes}")
        };
    }
    if node.primitive == Primitive::ScrollView {
        classes = if classes.is_empty() {
            "hozo-scroll-view".to_string()
        } else {
            format!("hozo-scroll-view {classes}")
        };
    }

    // `className`, not `class` -- Hozo's Web output is consumed as JSX
    // (the Vite plugin splices it back into React source), not raw HTML.
    //
    // Anything the parser couldn't decompose statically (proposal §7's
    // third tier) is concatenated back on at runtime. What it evaluates to
    // is matched by the project-wide candidate stylesheet
    // (`render_candidate_stylesheet`) -- the browser's own CSS engine does
    // the resolution, with no runtime code involved.
    let mut attrs = if node.class_name_fallback.is_empty() {
        if classes.is_empty() {
            String::new()
        } else {
            format!(r#" className="{classes}""#)
        }
    } else {
        for expr_ref in &node.class_name_fallback {
            diagnostics.push(hozo_ir::Diagnostic {
                code: hozo_ir::DiagnosticCode::DynamicClassNameNotResolved,
                severity: hozo_ir::Severity::Warning,
                message: format!(
                    "`{}` can't be resolved at build time, so it's passed through and its CSS \
                     comes from the project-wide candidate stylesheet instead. Only classes \
                     whose text appears literally somewhere in the project are covered -- one \
                     assembled at runtime (`` `bg-${{color}}-500` ``) still won't be.",
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
        // `hozoInteractive` supplies it, together with everything else it
        // has to agree with.
        if (tag == "Pressable" || synthesized_control) && *key == "tabIndex" {
            continue;
        }
        let key = if tag == "Pressable" && *key == "role" { "accessibilityRole" } else { key };
        match value {
            markup::AttrValue::Text(text) => attrs.push_str(&format!(r#" {key}="{text}""#)),
            markup::AttrValue::Expression(expr) => attrs.push_str(&format!(" {key}={{{expr}}}")),
        }
    }

    if let Some(value) = node.props.test_id {
        let name = if is_hozo_component { "testID" } else { "data-testid" };
        attrs.push_str(&format!(" {name}={{{}}}", source_text(source, value)));
    }
    if let Some(value) = node.props.native_id {
        let name = if is_hozo_component { "nativeID" } else { "id" };
        attrs.push_str(&format!(" {name}={{{}}}", source_text(source, value)));
    }
    if let Some(value) = node.props.pointer_events {
        let name = if is_hozo_component { "pointerEvents" } else { "data-hozo-pointer-events" };
        attrs.push_str(&format!(" {name}={{{}}}", source_text(source, value)));
    }
    if let Some(value) = node.props.accessibility_state {
        let value = source_text(source, value);
        if is_hozo_component {
            attrs.push_str(&format!(" accessibilityState={{{value}}}"));
        } else {
            // Only the keys the author actually wrote.
            //
            // This used to emit all five unconditionally, which is a type
            // error the moment the value is an object literal with fewer:
            // `accessibilityState={{ expanded: open }}` produced
            // `aria-disabled={({ expanded: open }).disabled}`, and
            // `Property 'disabled' does not exist` -- four times, in the
            // author's own build. A partial state object is the normal way
            // to write one.
            //
            // `None` keys means the expression is opaque (a variable, a
            // spread). Reading any of the five off that is fine: React
            // Native's `AccessibilityState` declares all of them optional.
            for (attr_name, key) in ARIA_STATE_ATTRS {
                // `disabled` folds into `hozoInteractive` when there is
                // one; emitting it here too put `aria-disabled` on the
                // element twice and let the later win by accident.
                if synthesized_control && *key == "disabled" {
                    continue;
                }
                let written = node
                    .props
                    .accessibility_state_keys
                    .as_ref()
                    .is_none_or(|keys| keys.iter().any(|written| written == key));
                if written {
                    attrs.push_str(&format!(" {attr_name}={{({value}).{key}}}"));
                }
            }
        }
    }
    if let Some(value) = node.props.accessibility_value {
        let value = source_text(source, value);
        if is_hozo_component {
            attrs.push_str(&format!(" accessibilityValue={{{value}}}"));
        } else {
            attrs.push_str(&format!(" aria-valuemin={{({value}).min}}"));
            attrs.push_str(&format!(" aria-valuemax={{({value}).max}}"));
            attrs.push_str(&format!(" aria-valuenow={{({value}).now}}"));
            attrs.push_str(&format!(" aria-valuetext={{({value}).text}}"));
        }
    }
    if let Some(value) = node.props.accessibility_live_region {
        let value = source_text(source, value);
        if is_hozo_component {
            attrs.push_str(&format!(" accessibilityLiveRegion={{{value}}}"));
        } else {
            attrs.push_str(&format!(" aria-live={{{value} === 'none' ? undefined : {value}}}"));
        }
    }
    if let Some(value) = node.props.on_layout {
        attrs.push_str(&format!(" onLayout={{{}}}", source_text(source, value)));
    }
    if tag == "Heading" {
        if let Some(level) = &node.props.heading_level {
            let value = match level {
                hozo_ir::HeadingLevel::Static(level) => level.to_string(),
                hozo_ir::HeadingLevel::Dynamic(expr) => source_text(source, *expr).to_string(),
            };
            attrs.push_str(&format!(" level={{{value}}}"));
        }
    }
    if tag == "List" {
        if let Some(ordered) = &node.props.list_ordered {
            attrs.push_str(&format!(" ordered={{{}}}", render_condition_expr(source, ordered)));
        }
    }

    // Written under the DOM's name for it. See the parser: the source may
    // spell this either way, and re-emitting `accessibilityLabel` here
    // would leave the field with no accessible name at all.
    if node.primitive != Primitive::Image {
      if let Some(label) = node.props.accessibility_label {
        // `aria-label` for a real DOM element; `accessibilityLabel` for a
        // Hozo component, which maps it to `aria-label` itself. Writing
        // the DOM spelling on a component would make it an unknown prop
        // that React drops.
        let name = if is_hozo_component {
            "accessibilityLabel"
        } else {
            "aria-label"
        };
          attrs.push_str(&format!(" {name}={{{}}}", source_text(source, label)));
      }
    }
    if let Some(hint) = node.props.accessibility_hint {
        let name = if is_hozo_component {
            "accessibilityHint"
        } else {
            "aria-description"
        };
        attrs.push_str(&format!(" {name}={{{}}}", source_text(source, hint)));
    }
    if let Some(src) = node.props.image_src {
        attrs.push_str(&format!(" src={{{}}}", source_text(source, src)));
    }
    if let Some(src) = node.props.image_default_source {
        attrs.push_str(&format!(" defaultSource={{{}}}", source_text(source, src)));
    }
    if let Some(horizontal) = &node.props.scroll_horizontal {
        let horizontal = render_condition_expr(source, horizontal);
        if matches!(tag, "ScrollView" | "FlatList") {
            attrs.push_str(&format!(" horizontal={{{horizontal}}}"));
        } else {
            attrs.push_str(&format!(" data-hozo-horizontal={{{horizontal} ? '' : undefined}}"));
        }
    }
    if node.primitive == Primitive::ScrollView {
        if tag == "ScrollView" {
            if let Some(refreshing) = &node.props.refreshing {
                attrs.push_str(&format!(" refreshing={{{}}}", render_condition_expr(source, refreshing)));
            }
            if let Some(on_refresh) = node.props.on_refresh {
                attrs.push_str(&format!(" onRefresh={{{}}}", source_text(source, on_refresh)));
            }
            if let Some(value) = node.props.keyboard_should_persist_taps {
                attrs.push_str(&format!(" keyboardShouldPersistTaps={{{}}}", source_text(source, value)));
            }
            if let Some(value) = &node.props.shows_vertical_scroll_indicator {
                attrs.push_str(&format!(" showsVerticalScrollIndicator={{{}}}", render_condition_expr(source, value)));
            }
            if let Some(value) = &node.props.shows_horizontal_scroll_indicator {
                attrs.push_str(&format!(" showsHorizontalScrollIndicator={{{}}}", render_condition_expr(source, value)));
            }
        } else if node.props.shows_vertical_scroll_indicator.is_some()
            || node.props.shows_horizontal_scroll_indicator.is_some()
        {
            let horizontal = node.props.scroll_horizontal.as_ref()
                .map(|value| render_condition_expr(source, value))
                .unwrap_or_else(|| "false".to_string());
            let vertical = node.props.shows_vertical_scroll_indicator.as_ref()
                .map(|value| render_condition_expr(source, value))
                .unwrap_or_else(|| "true".to_string());
            let horizontal_indicator = node.props.shows_horizontal_scroll_indicator.as_ref()
                .map(|value| render_condition_expr(source, value))
                .unwrap_or_else(|| "true".to_string());
            attrs.push_str(&format!(
                " data-hozo-hide-scrollbar={{({horizontal} ? !({horizontal_indicator}) : !({vertical})) ? '' : undefined}}"
            ));
        }
    } else if node.primitive == Primitive::FlatList {
        if let Some(refreshing) = &node.props.refreshing {
            attrs.push_str(&format!(" refreshing={{{}}}", render_condition_expr(source, refreshing)));
        }
        if let Some(on_refresh) = node.props.on_refresh {
            attrs.push_str(&format!(" onRefresh={{{}}}", source_text(source, on_refresh)));
        }
        if let Some(value) = node.props.keyboard_should_persist_taps {
            attrs.push_str(&format!(" keyboardShouldPersistTaps={{{}}}", source_text(source, value)));
        }
        if let Some(value) = &node.props.shows_vertical_scroll_indicator {
            attrs.push_str(&format!(" showsVerticalScrollIndicator={{{}}}", render_condition_expr(source, value)));
        }
        if let Some(value) = &node.props.shows_horizontal_scroll_indicator {
            attrs.push_str(&format!(" showsHorizontalScrollIndicator={{{}}}", render_condition_expr(source, value)));
        }
    }
    if matches!(node.primitive, Primitive::ScrollView | Primitive::FlatList) {
        if let Some(value) = node.props.on_scroll {
            attrs.push_str(&format!(" onScroll={{{}}}", source_text(source, value)));
        }
        if let Some(value) = node.props.scroll_event_throttle {
            attrs.push_str(&format!(" scrollEventThrottle={{{}}}", source_text(source, value)));
        }
    }
    if node.primitive == Primitive::Image {
        if let Some(label) = node.props.accessibility_label {
            attrs.push_str(&format!(" alt={{{}}}", source_text(source, label)));
        }
    }
    if let Some(open) = &node.props.open {
        attrs.push_str(&format!(" open={{{}}}", render_condition_expr(source, open)));
    }
    if let Some(on_press) = node.props.on_press {
        let name = if tag == "Pressable" { "onPress" } else { "onClick" };
        // Keyed off `tabIndex` being in the emitted attributes rather than
        // off the primitive: that attribute *is* the statement that Hozo
        // made this element focusable itself, so "Hozo put it in the tab
        // order" and "Hozo owes it the rest of being a control" cannot
        // drift apart.
        if synthesized_control {
            // One call, not five expressions. `disabled` means five things
            // at once (see docs/decisions/001) and emitting them
            // separately is how they came apart: this announced
            // `aria-disabled` and then ran the handler anyway, and once
            // keyboard activation existed it ran on Enter and Space too.
            //
            // It also means the guard expression is evaluated once rather
            // than once per thing it decides.
            attrs.push_str(&format!(
                " {{...hozoInteractive({}{})}}",
                source_text(source, on_press),
                disabled_expr
                    .as_ref()
                    .map(|expr| format!(", {expr}"))
                    .unwrap_or_default(),
            ));
            *uses_key_activation = true;
        } else {
            attrs.push_str(&format!(" {name}={{{}}}", source_text(source, on_press)));
        }
    }
    for (name, value) in [
        ("onStartShouldSetResponder", node.props.on_start_should_set_responder),
        ("onStartShouldSetResponderCapture", node.props.on_start_should_set_responder_capture),
        ("onMoveShouldSetResponder", node.props.on_move_should_set_responder),
        ("onMoveShouldSetResponderCapture", node.props.on_move_should_set_responder_capture),
        ("onResponderGrant", node.props.on_responder_grant),
        ("onResponderStart", node.props.on_responder_start),
        ("onResponderMove", node.props.on_responder_move),
        ("onResponderEnd", node.props.on_responder_end),
        ("onResponderRelease", node.props.on_responder_release),
        ("onResponderReject", node.props.on_responder_reject),
        ("onResponderTerminate", node.props.on_responder_terminate),
        ("onResponderTerminationRequest", node.props.on_responder_termination_request),
    ] {
        if let Some(value) = value {
            attrs.push_str(&format!(" {name}={{{}}}", source_text(source, value)));
        }
    }
    if let Some(disabled) = node.props.disabled.as_ref().filter(|_| !synthesized_control) {
        // `disabled` is a real, React-boolean-aware HTML attribute only on
        // actual form controls (<button> here) -- react omits it entirely
        // when the value is falsy. Everything else Hozo maps to a <div>
        // (Pressable, View, Text), where the native attribute has no
        // effect at all, so ARIA is the honest choice there instead.
        let attr_name = if node.primitive == Primitive::Button || tag == "Pressable" { "disabled" } else { "aria-disabled" };
        attrs.push_str(&format!(" {attr_name}={{{}}}", render_condition_expr(source, disabled)));
    }

    // CSS attribute selectors (`[data-hozo-cond-x-y]`, built in css.rs)
    // match on an attribute's *presence*, not its string value -- so the
    // guard must be wired as `{expr ? '' : undefined}` (React omits
    // `undefined`-valued attributes entirely) rather than a literal
    // "true"/"false" string, which would stay present either way and
    // permanently match the selector.
    for expr_ref in collect_expr_refs(node) {
        let guard = source_text(source, expr_ref);
        attrs.push_str(&format!(" {}={{{guard} ? '' : undefined}}", css::expr_ref_attribute(expr_ref)));
    }

    // Everything Hozo doesn't model, re-emitted verbatim and last so JSX's
    // last-wins duplicate resolution keeps matching the source's own
    // ordering semantics. Known cross-platform props were consumed above;
    // an unknown RN-specific prop is still carried as written, because a
    // visible React warning is safer than silently deleting app behavior.
    for prop in &node.props.passthrough {
        attrs.push(' ');
        attrs.push_str(&render_verbatim(
            prop.span,
            &prop.nested,
            source,
            theme,
            allocator,
            rules,
            diagnostics,
            uses_view_base,
            uses_key_activation,
        ));
    }

    // In source order, and every child emitted -- `Verbatim` covers the
    // ones the compiler doesn't model, re-emitted from source rather than
    // deleted. Order is load-bearing: `<Text>Hello {name}</Text>` and
    // `<Text>{name} Hello</Text>` differ only in it.
    let inner: String = node
        .children
        .iter()
        .map(|child| match child {
            hozo_ir::Child::Node(child_node) => {
                render_node(child_node, source, theme, allocator, rules, diagnostics, uses_view_base, uses_key_activation)
            }
            hozo_ir::Child::Text(text) => markup::html_escape(text),
            hozo_ir::Child::Verbatim { source: expr_ref, nested } => render_verbatim(
                *expr_ref,
                nested,
                source,
                theme,
                allocator,
                rules,
                diagnostics,
                uses_view_base,
                uses_key_activation,
            ),
        })
        .collect();

    // `<input>` is a void element: HTML forbids a closing tag and React
    // throws on children. Nothing can be inside one, so there is no inner
    // to lose by self-closing.
    if tag == "input" || tag == "img" {
        return format!("<{tag}{attrs} />");
    }
    format!("<{tag}{attrs}>{inner}</{tag}>")
}

/// Re-emits a carried expression from source, with each Hozo primitive
/// inside it replaced by its lowered output.
///
/// The nested spans are subranges of `expr_ref`'s and don't overlap (each
/// is the outermost primitive on its branch), so one left-to-right pass is
/// enough. `{show && <Text className="p-4">hi</Text>}` comes out as
/// `{show && <span className="hozo-1">hi</span>}` -- the guard untouched,
/// the element fully compiled.
fn render_verbatim(
    expr_ref: hozo_ir::ExprRef,
    nested: &[hozo_ir::NestedNode],
    source: &str,
    theme: &Theme,
    allocator: &mut ClassAllocator,
    rules: &mut String,
    diagnostics: &mut Vec<Diagnostic>,
    uses_view_base: &mut bool,
    uses_key_activation: &mut bool,
) -> String {
    let start = expr_ref.0.start as usize;
    let mut out = String::new();
    let mut cursor = start;
    for entry in nested {
        let from = entry.span.start as usize;
        out.push_str(&source[cursor..from]);
        out.push_str(&render_node(
            &entry.node,
            source,
            theme,
            allocator,
            rules,
            diagnostics,
            uses_view_base,
            uses_key_activation,
        ));
        cursor = entry.span.end as usize;
    }
    out.push_str(&source[cursor..expr_ref.0.end as usize]);
    out
}

fn collect_expr_refs(node: &Node) -> Vec<hozo_ir::ExprRef> {
    let mut refs = Vec::new();
    for decl in &node.style {
        if let hozo_ir::Condition::Expr(expr) = &decl.condition {
            collect_from_expr(expr, &mut refs);
        }
    }
    refs.sort_by_key(|r: &hozo_ir::ExprRef| (r.0.start, r.0.end));
    refs.dedup();
    refs
}

fn collect_from_expr(expr: &hozo_ir::ConditionExpr, out: &mut Vec<hozo_ir::ExprRef>) {
    use hozo_ir::ConditionExpr;
    match expr {
        ConditionExpr::Static(_) => {}
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
import { View, Text, Button } from '@hozo/core'

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
        let parsed = hozo_parser::parse_tsx(LOGIN_EXAMPLE);
        let root = &parsed.roots[0].node;
        let output = lower(root, LOGIN_EXAMPLE, &Theme::default());

        assert!(output.jsx.starts_with(r#"<div className="hozo-view hozo-0">"#));
        assert!(output.jsx.contains("<span className=\"hozo-1\">Welcome</span>"));
        assert!(output.jsx.contains("<button className=\"hozo-2\" type=\"button\">Continue</button>"));

        assert!(output.css.contains(".hozo-view {"));
        assert!(output.css.contains(".hozo-0 {"));
        assert!(output.css.contains("flex: 1 1 0%;"));
        assert!(output.css.contains("padding-top: 24px;"));
        assert!(output.css.contains(".hozo-1 {"));
        assert!(output.css.contains("font-size: 20px;"));
        assert!(output.css.contains("font-weight: 700;"));
        assert!(output.css.contains(".hozo-2 {"));
        // `px-4` is Tailwind's logical inline axis, not left/right.
        assert!(output.css.contains("padding-inline-start: 16px;"));
        assert!(output.css.contains("padding-inline-end: 16px;"));

        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn hover_condition_compiles_to_a_real_pseudo_class() {
        let source = r#"
            import { View } from '@hozo/core'
            const el = <View className="hover:text-xl" />
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.css.contains(".hozo-0:hover {"));
        assert!(output.css.contains("font-size: 20px;"));
    }

    #[test]
    fn an_unresolvable_class_name_is_preserved_not_dropped() {
        let source = r#"
            import { View } from '@hozo/core'
            const el = <View className={classNameFromProps} />
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());

        // The expression reaches the DOM instead of vanishing...
        assert!(output.jsx.contains("classNameFromProps"));
        // ...and the diagnostic says Hozo generates no CSS behind it,
        // rather than letting it look resolved.
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(
            output.diagnostics[0].code,
            hozo_ir::DiagnosticCode::DynamicClassNameNotResolved
        );
    }

    #[test]
    fn candidate_css_uses_the_real_tailwind_names() {
        // The classes a scan finds live somewhere the AST-based reader
        // can't see them, so the only thing that can match at runtime is
        // the name itself. No runtime code is involved: the browser's CSS
        // engine resolves it.
        let css = render_candidate_stylesheet(&["p-8".to_string(), "p-2".to_string()], &Theme::default());
        assert!(css.contains(".p-8 {"));
        assert!(css.contains(".p-2 {"));
        assert!(css.contains("padding-top: 32px;"));
    }

    #[test]
    fn candidate_css_is_not_emitted_per_file() {
        // Per-file lowering compiles everything readable into scoped rules
        // and stops there -- the candidate set is project-wide, so shipping
        // it from `lower` would put a copy in every file that needs any of
        // it.
        let source = r#"
            import { View } from '@hozo/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', getDynamic())} />
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.css.contains(".hozo-0 {"), "{}", output.css);
        assert!(!output.css.contains(".p-4 {"), "{}", output.css);
    }

    #[test]
    fn candidate_variant_classes_are_escaped_in_the_selector() {
        // `hover:bg-blue-500` contains selector syntax and has to be
        // written `.hover\:bg-blue-500:hover` to match literally.
        let css = render_candidate_stylesheet(&["hover:bg-blue-500".to_string()], &Theme::default());
        assert!(css.contains(r".hover\:bg-blue-500:hover {"));
    }

    #[test]
    fn unrecognized_candidates_are_skipped() {
        // Scanning is imprecise by design, so the stylesheet has to
        // tolerate tokens that only looked like classes.
        let css = render_candidate_stylesheet(&["useState".to_string(), "p-4".to_string()], &Theme::default());
        assert!(css.contains(".p-4 {"));
        assert!(!css.contains("useState"));
    }

    #[test]
    fn children_the_compiler_cannot_read_are_carried_and_their_primitives_still_compile() {
        // Until 2026-08-15 every one of these vanished from the output with
        // no diagnostic. Now the *expression* is carried untouched while
        // the primitives inside it compile exactly as top-level ones do.
        let source = r#"
            import { View, Text } from '@hozo/core'
            export function C({ show, items, name }) {
              return (
                <View className="p-4">
                  <Avatar />
                  {show && <Text className="text-xl">hi</Text>}
                  {items.map((i) => <Text key={i} className="p-2">{i}</Text>)}
                  <Text>Hello {name}</Text>
                </View>
              )
            }
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());

        // Carried, not interpreted: the guard and the callback survive.
        assert!(output.jsx.contains("{show && "), "{}", output.jsx);
        assert!(output.jsx.contains("{items.map((i) => "), "{}", output.jsx);
        assert!(output.jsx.contains("<Avatar />"), "{}", output.jsx);
        // ...while the primitives inside them are fully lowered.
        assert!(!output.jsx.contains("<Text"), "{}", output.jsx);
        assert!(output.css.contains("font-size: 20px;"), "{}", output.css);
        assert!(output.css.contains("padding-top: 8px;"), "{}", output.css);
        // Text and expression keep their order.
        assert!(output.jsx.contains(">Hello {name}<"), "{}", output.jsx);
    }

    #[test]
    fn flat_list_keeps_its_web_renderer_and_compiles_the_render_item_body() {
        let source = r#"
            import { FlatList, Text } from '@hozo/core'
            const el = <FlatList className="h-40" data={rows}
              renderItem={({ item }) => <Text className="p-2">{item}</Text>} />
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<FlatList className=\"hozo-0\""), "{}", output.jsx);
        assert!(output.jsx.contains("renderItem={({ item }) => <span className=\"hozo-1\">{item}</span>}"), "{}", output.jsx);
        assert!(output.css.contains("height: 160px"), "{}", output.css);
        assert!(output.css.contains("padding-top: 8px"), "{}", output.css);
    }

    #[test]
    fn flat_list_keeps_refresh_columns_and_pagination_props_for_its_web_runtime() {
        let source = r#"
            import { FlatList, Text } from '@hozo/core'
            const el = <FlatList data={rows} horizontal={horizontal} numColumns={2}
              refreshing={loading} onRefresh={reload}
              onEndReached={loadMore} onEndReachedThreshold={0.5}
              showsHorizontalScrollIndicator={false}
              ListEmptyComponent={<Text className="p-2">Empty</Text>}
              renderItem={({ item }) => <Text>{item}</Text>} />
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<FlatList"), "{}", output.jsx);
        assert!(output.jsx.contains(" horizontal={horizontal}"), "{}", output.jsx);
        assert!(output.jsx.contains(" refreshing={loading} onRefresh={reload}"), "{}", output.jsx);
        assert!(output.jsx.contains(" showsHorizontalScrollIndicator={false}"), "{}", output.jsx);
        assert!(output.jsx.contains("numColumns={2}"), "{}", output.jsx);
        assert!(output.jsx.contains("onEndReached={loadMore}"), "{}", output.jsx);
        assert!(output.jsx.contains("onEndReachedThreshold={0.5}"), "{}", output.jsx);
        assert!(output.jsx.contains("ListEmptyComponent={<span className=\"hozo-1\">Empty</span>}"), "{}", output.jsx);
    }

    #[test]
    fn scroll_view_refresh_uses_the_accessible_web_fallback() {
        let source = r#"
            import { ScrollView, Text } from '@hozo/core'
            const el = <ScrollView className="h-40" horizontal={wide}
              refreshing={loading} onRefresh={reload}
              keyboardShouldPersistTaps="handled"
              showsHorizontalScrollIndicator={false}
              accessibilityLabel="Results">
              <Text>row</Text>
            </ScrollView>
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<ScrollView className=\"hozo-scroll-view hozo-0\""), "{}", output.jsx);
        assert!(output.jsx.contains(" horizontal={wide}"), "{}", output.jsx);
        assert!(output.jsx.contains(" refreshing={loading} onRefresh={reload}"), "{}", output.jsx);
        assert!(output.jsx.contains(" keyboardShouldPersistTaps={\"handled\"}"), "{}", output.jsx);
        assert!(output.jsx.contains(" showsHorizontalScrollIndicator={false}"), "{}", output.jsx);
        assert!(output.jsx.contains(" accessibilityLabel={\"Results\"}"), "{}", output.jsx);
        assert!(!output.jsx.contains("aria-label"), "{}", output.jsx);
    }

    #[test]
    fn scroll_view_without_refresh_stays_zero_runtime_and_hides_its_active_indicator() {
        let source = r#"
            import { ScrollView } from '@hozo/core'
            const el = <ScrollView horizontal={wide}
              showsHorizontalScrollIndicator={showHorizontal}
              showsVerticalScrollIndicator={showVertical} />
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<div className=\"hozo-scroll-view\""), "{}", output.jsx);
        assert!(output.jsx.contains("data-hozo-horizontal={wide ? '' : undefined}"), "{}", output.jsx);
        assert!(output.jsx.contains("data-hozo-hide-scrollbar={(wide ? !(showHorizontal) : !(showVertical)) ? '' : undefined}"), "{}", output.jsx);
        assert!(output.css.contains("scrollbar-width: none"), "{}", output.css);
        assert!(output.css.contains("::-webkit-scrollbar"), "{}", output.css);
    }

    #[test]
    fn universal_native_props_lower_to_dom_identity_pointer_and_aria() {
        let source = r#"
            import { View } from '@hozo/core'
            const el = <View testID="card" nativeID="result-card" pointerEvents="box-none"
              accessibilityState={{ disabled, selected: true, busy }}
              accessibilityValue={{ min: 0, max: 10, now: progress, text: label }}
              accessibilityLiveRegion="polite" />
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<div className=\"hozo-view\""), "{}", output.jsx);
        assert!(output.jsx.contains("data-testid={\"card\"}"), "{}", output.jsx);
        assert!(output.jsx.contains("id={\"result-card\"}"), "{}", output.jsx);
        assert!(output.jsx.contains("data-hozo-pointer-events={\"box-none\"}"), "{}", output.jsx);
        assert!(output.jsx.contains("aria-disabled={({ disabled, selected: true, busy }).disabled}"), "{}", output.jsx);
        assert!(output.jsx.contains("aria-valuenow={({ min: 0, max: 10, now: progress, text: label }).now}"), "{}", output.jsx);
        assert!(output.jsx.contains("aria-live={\"polite\" === 'none' ? undefined : \"polite\"}"), "{}", output.jsx);
        assert!(output.css.contains("[data-hozo-pointer-events='box-none'] > *"), "{}", output.css);
    }

    #[test]
    fn on_layout_selects_the_core_adapter_only_for_the_measured_node() {
        let source = r#"
            import { View, Text } from '@hozo/core'
            const el = <View onLayout={measure} testID="measured"><Text>child</Text></View>
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<View className=\"hozo-view\""), "{}", output.jsx);
        assert!(output.jsx.contains("testID={\"measured\"} onLayout={measure}"), "{}", output.jsx);
        assert!(output.jsx.contains("<span>child</span>"), "{}", output.jsx);
    }

    #[test]
    fn responder_callbacks_select_the_pointer_bridge_only_for_interactive_nodes() {
        let source = r#"
            import { View, Pressable } from '@hozo/core'
            const el = <View onStartShouldSetResponder={wantStart}
              onStartShouldSetResponderCapture={captureStart}
              onMoveShouldSetResponder={wantMove}
              onMoveShouldSetResponderCapture={captureMove}
              onResponderGrant={grant} onResponderStart={start}
              onResponderMove={move} onResponderEnd={end}
              onResponderRelease={release} onResponderReject={reject}
              onResponderTerminate={terminate}
              onResponderTerminationRequest={allowTermination}>
                <Pressable accessibilityRole="button" disabled={locked}
                  onPress={save} onResponderGrant={pressGrant} />
            </View>
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<View className=\"hozo-view\""), "{}", output.jsx);
        for expected in [
            "onStartShouldSetResponder={wantStart}",
            "onStartShouldSetResponderCapture={captureStart}",
            "onMoveShouldSetResponder={wantMove}",
            "onMoveShouldSetResponderCapture={captureMove}",
            "onResponderGrant={grant}",
            "onResponderStart={start}",
            "onResponderMove={move}",
            "onResponderEnd={end}",
            "onResponderRelease={release}",
            "onResponderReject={reject}",
            "onResponderTerminate={terminate}",
            "onResponderTerminationRequest={allowTermination}",
        ] {
            assert!(output.jsx.contains(expected), "missing {expected}: {}", output.jsx);
        }
        assert!(output.jsx.contains("<Pressable accessibilityRole=\"button\""), "{}", output.jsx);
        assert!(output.jsx.contains("onPress={save}"), "{}", output.jsx);
        assert!(output.jsx.contains("disabled={locked}"), "{}", output.jsx);
        assert!(!output.jsx.contains("onClick={save}"), "{}", output.jsx);
    }

    #[test]
    fn pan_responder_handler_spreads_select_the_web_bridge() {
        let source = r#"
            import { View, PanResponder } from '@hozo/core'
            const pan = PanResponder.create({ onMoveShouldSetPanResponder: () => true })
            const el = <View className="p-4" {...pan.panHandlers} />
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<View className=\"hozo-view hozo-0\""), "{}", output.jsx);
        assert!(output.jsx.contains("{...pan.panHandlers}"), "{}", output.jsx);
    }

    #[test]
    fn structured_and_fallback_image_sources_select_the_web_normalizer() {
        let source = r#"
            import { Image } from '@hozo/core'
            const el = <Image src={{ uri: remote }} defaultSource={assetModule} alt="Cover" />
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<Image "), "{}", output.jsx);
        assert!(output.jsx.contains("src={{ uri: remote }}"), "{}", output.jsx);
        assert!(output.jsx.contains("defaultSource={assetModule}"), "{}", output.jsx);
        assert!(output.jsx.contains("alt={\"Cover\"}"), "{}", output.jsx);
    }

    #[test]
    fn scroll_events_select_the_bridge_and_keep_the_native_contract() {
        let source = r#"
            import { ScrollView, Text } from '@hozo/core'
            const el = <ScrollView onScroll={remember} scrollEventThrottle={16}><Text>row</Text></ScrollView>
        "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.starts_with("<ScrollView className=\"hozo-scroll-view\""), "{}", output.jsx);
        assert!(output.jsx.contains("onScroll={remember} scrollEventThrottle={16}"), "{}", output.jsx);
    }

    #[test]
    fn only_the_unresolvable_leaf_falls_back() {
        // proposal §7's three tiers in one className: a literal compiles
        // away, a guarded literal becomes a conditional rule, and only the
        // opaque call is passed through.
        let source = r#"
            import { View } from '@hozo/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', active && 'text-xl', getDynamic())} />
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());

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
            import { View } from '@hozo/core'
            const el = <View className="space-x-2" />
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.css.contains(":where(.hozo-0 > :not(:last-child)) {"));
        assert!(output.css.contains("margin-inline-end: 8px;"));
        // Not on the element itself.
        assert!(!output.css.contains(".hozo-0 {\n  margin-inline-end"));
    }

    /// Compiles one element with `classes` and returns its CSS.
    fn css_for(classes: &str) -> String {
        let source = format!(
            "import {{ View }} from '@hozo/core'\nconst el = <View className=\"{classes}\" />\n"
        );
        let parsed = hozo_parser::parse_tsx(&source);
        lower(&parsed.roots[0].node, &source, &Theme::default()).css
    }

    #[test]
    fn the_translate_axes_share_one_declaration() {
        // CSS `translate` is one property taking up to three values. Until
        // 2026-08-15 each axis emitted its own `translate:`, so this pair
        // wrote two declarations and last-wins threw the x away. The
        // conformance suite can't see it -- it compiles one utility at a
        // time, and each was correct alone.
        assert!(css_for("translate-x-4").contains("translate: 16px 0;"));
        assert!(css_for("translate-y-8").contains("translate: 0 32px;"));
        let css = css_for("translate-x-4 translate-y-8");
        assert!(css.contains("translate: 16px 32px;"), "{css}");
        assert_eq!(css.matches("translate:").count(), 1, "{css}");
    }

    #[test]
    fn scrollbar_thumb_and_track_share_one_declaration() {
        // `scrollbar-color` takes both halves at once, so writing only one
        // still has to name the other -- Tailwind's registers default to
        // transparent, not to the UA's own colours.
        assert!(css_for("scrollbar-thumb-red-500")
            .contains("scrollbar-color: oklch(63.7% 0.237 25.331) #0000;"));
        assert!(css_for("scrollbar-track-blue-500")
            .contains("scrollbar-color: #0000 oklch(62.3% 0.214 259.815);"));

        let css = css_for("scrollbar-thumb-red-500 scrollbar-track-blue-500");
        assert!(
            css.contains(
                "scrollbar-color: oklch(63.7% 0.237 25.331) oklch(62.3% 0.214 259.815);"
            ),
            "{css}"
        );
    }

    #[test]
    fn mask_utilities_compose_into_one_resolved_layer_list() {
        // Tailwind assembles `mask-image` from `--tw-mask-*` registers, so
        // several utilities contribute to one declaration. The conformance
        // suite only ever compiles one at a time, so the combinations are
        // pinned here instead.
        let css = css_for("mask-x-from-4 mask-x-to-80%");
        assert!(
            css.contains(
                "mask-image: linear-gradient(to left, black 16px, transparent 80%), \
                 linear-gradient(to right, black 16px, transparent 80%), \
                 linear-gradient(#fff, #fff), linear-gradient(#fff, #fff), \
                 linear-gradient(#fff, #fff), linear-gradient(#fff, #fff);"
            ),
            "{css}"
        );

        // An angle and a stop land in the same gradient.
        let css = css_for("mask-linear-45 mask-linear-from-red-500");
        assert!(
            css.contains(
                "mask-image: linear-gradient(45deg, oklch(63.7% 0.237 25.331) 0%, \
                 transparent 100%), linear-gradient(#fff, #fff), linear-gradient(#fff, #fff);"
            ),
            "{css}"
        );

        // The radial shaping utilities paint nothing alone but change the
        // gradient another utility supplies.
        let css = css_for("mask-radial-from-4 mask-circle mask-radial-at-top");
        assert!(css.contains("radial-gradient(circle farthest-corner at top, "), "{css}");
    }

    #[test]
    fn a_mask_composite_alone_emits_no_layer_list() {
        // `mask-add` with nothing to composite is just the composite mode;
        // emitting a fully-opaque `mask-image` would be a no-op declaration
        // Tailwind doesn't write either.
        let css = css_for("mask-add");
        assert!(css.contains("mask-composite: add;"), "{css}");
        assert!(!css.contains("mask-image"), "{css}");
    }

    #[test]
    fn radial_shaping_alone_paints_nothing() {
        assert!(!css_for("mask-circle").contains("mask-image"));
    }

    #[test]
    fn placeholder_colour_scopes_itself_to_the_pseudo_element() {
        // The conformance suite cannot catch this: it compares
        // declarations, and the difference between tinting the placeholder
        // and tinting the real text lives entirely in the selector. Emitted
        // as a plain `color` this would score a false match while doing the
        // wrong thing.
        let css = css_for("placeholder-red-500");
        assert!(css.contains(".hozo-0::placeholder {"), "{css}");
        assert!(!css.contains(".hozo-0 {"), "{css}");
    }

    #[test]
    fn divide_becomes_a_child_scoped_rule_like_space() {
        // `divide-*` styles the gaps *between* children, so like `space-*`
        // it can't be a declaration on the element itself.
        let css = css_for("divide-y-4");
        assert!(css.contains(":where(.hozo-0 > :not(:last-child)) {"), "{css}");
        // Tailwind writes both edges, zeroing the leading one, so
        // `divide-*-reverse` can flip which edge carries the border without
        // a different rule -- matched here so the output stays identical.
        assert!(css.contains("border-top-width: 0;"), "{css}");
        assert!(css.contains("border-bottom-width: 4px;"), "{css}");
    }

    #[test]
    fn outline_width_carries_a_style_so_it_actually_renders() {
        // Same reason border widths do: CSS defaults `outline-style` to
        // `none`, so a width alone shows nothing.
        let css = css_for("outline-2");
        assert!(css.contains("outline-style: solid;"), "{css}");
        assert!(css.contains("outline-width: 2px;"), "{css}");
    }

    #[test]
    fn ring_and_shadow_compose_into_one_box_shadow() {
        // The whole point of keeping these as separate IR properties. A
        // single `BoxShadow` would make the later utility win under
        // `dedupe_last_wins`, and the conformance suite can't catch it --
        // it compares one utility at a time, and a ring colour paints
        // nothing on its own.
        let css = css_for("ring-2 ring-blue-500 shadow-lg");
        assert!(
            css.contains(
                "box-shadow: 0 0 0 2px oklch(62.3% 0.214 259.815), \
                 0 10px 15px -3px rgb(0 0 0 / 0.1), 0 4px 6px -4px rgb(0 0 0 / 0.1);"
            ),
            "{css}"
        );
    }

    #[test]
    fn a_ring_defaults_to_currentcolor_and_stacks_inside_out() {
        // Tailwind's default ring colour, and its layer order: inset ring
        // before ring before shadow.
        let css = css_for("inset-ring-4 ring-2");
        assert!(
            css.contains("box-shadow: inset 0 0 0 4px currentcolor, 0 0 0 2px currentcolor;"),
            "{css}"
        );
    }

    #[test]
    fn shadow_none_clears_the_shadow_without_taking_the_ring_with_it() {
        assert!(css_for("shadow-none").contains("box-shadow: none;"));
        // `none` as the whole declaration would erase the ring too, which
        // is not what Tailwind's register-clearing does.
        let css = css_for("shadow-none ring-2");
        assert!(css.contains("box-shadow: 0 0 0 2px currentcolor;"), "{css}");
    }

    #[test]
    fn a_ring_colour_alone_paints_nothing() {
        // Correct, not a gap: there is no width to paint. Tailwind emits
        // only a custom property here for the same reason.
        assert!(!css_for("ring-blue-500").contains("box-shadow"));
    }

    #[test]
    fn animation_emits_its_keyframes_once() {
        let source = r#"
            import { View, Text } from '@hozo/core'
            const el = (
              <View className="animate-spin">
                <Text className="animate-spin">x</Text>
              </View>
            )
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.css.contains("animation: spin 1s linear infinite;"));
        // An `animation` declaration is inert without its keyframes, and
        // two users of the same animation must not duplicate the block.
        assert_eq!(output.css.matches("@keyframes spin").count(), 1);
    }

    #[test]
    fn unmodeled_props_and_spreads_reach_the_output() {
        let source = r#"
            import { View } from '@hozo/core'
            const el = <View className="p-4" {...rest} onLayout={onLayout} testID="row" />
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.contains("{...rest}"));
        assert!(output.jsx.contains("onLayout={onLayout}"));
        assert!(output.jsx.starts_with("<View "), "{}", output.jsx);
        assert!(output.jsx.contains(r#"testID={"row"}"#));
    }

    #[test]
    fn pressed_condition_compiles_to_a_real_active_pseudo_class() {
        let source = r#"
            import { Button } from '@hozo/core'
            const el = <Button className="pressed:opacity-50">Save</Button>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.css.contains(".hozo-0:active {"));
        assert!(output.css.contains("opacity: 0.5;"));
    }

    #[test]
    fn interactive_pressable_without_role_is_diagnosed_from_real_source() {
        // Previously only reachable by hand-constructing a `Node` directly
        // -- `PropSet.on_press`/`accessibility_role` weren't populated by
        // the parser at all until hozo_parser::jsx gained onPress/
        // accessibilityRole attribute parsing.
        let source = r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable onPress={handleTap}>Tap</Pressable>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert_eq!(output.diagnostics.len(), 1);
        assert_eq!(output.diagnostics[0].code, hozo_ir::DiagnosticCode::A11yInteractiveWithoutRole);
        assert!(!output.jsx.contains("role="));
        // The press is wired regardless of the diagnostic -- warning about
        // a missing role is not a reason to drop the handler. It goes
        // through `hozoInteractive` because Hozo still had to synthesize
        // the control, role or no role.
        assert!(output.jsx.contains("{...hozoInteractive(handleTap)}"), "{}", output.jsx);
    }

    #[test]
    fn accessibility_role_suppresses_the_diagnostic_and_sets_role() {
        let source = r#"
            import { Pressable } from '@hozo/core'
            const el = (
              <Pressable onPress={handleTap} accessibilityRole="button">Tap</Pressable>
            )
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.diagnostics.is_empty());
        assert!(output.jsx.contains(r#"role="button""#));
    }

    #[test]
    fn disabled_renders_the_native_attribute_on_button() {
        let source = r#"
            import { Button } from '@hozo/core'
            const el = <Button disabled={isLoading}>Save</Button>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.contains("disabled={isLoading}"));
        assert!(!output.jsx.contains("aria-disabled"));
    }

    #[test]
    fn boolean_disabled_renders_the_attribute_and_drives_the_variant() {
        let source = r#"
            import { Button } from '@hozo/core'
            const el = <Button disabled className="disabled:opacity-50">Save</Button>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert!(output.jsx.contains("disabled={true}"), "{}", output.jsx);
        assert!(output.css.contains(":disabled"), "{}", output.css);
    }

    #[test]
    fn disabled_renders_aria_disabled_on_pressable() {
        // Pressable is a <div> -- the native `disabled` attribute has no
        // effect there, so this must be ARIA instead.
        let source = r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable disabled={isLoading} accessibilityRole="button">Save</Pressable>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.contains("aria-disabled={isLoading}"));
    }

    #[test]
    fn dynamic_class_name_guard_is_wired_as_a_presence_toggle() {
        let source = r#"
            import { View } from '@hozo/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', active && 'text-xl')} />
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());

        // The guard is re-emitted verbatim, wired to toggle attribute
        // *presence* (not a literal "true"/"false" string, which would
        // permanently match the CSS attribute selector either way).
        assert!(output.jsx.contains("={active ? '' : undefined}"));
        assert!(!output.jsx.contains(r#"="false""#));
        assert!(!output.jsx.contains(r#"="true""#));

        // And the CSS selector that attribute name feeds is present too.
        assert!(output.css.contains("] {"));
    }

    #[test]
    fn semantic_primitives_lower_to_native_html_elements() {
        let source = r#"
            import { Section, Heading, Paragraph } from '@hozo/core'
            const el = <Section><Heading level={2}>Title</Heading><Paragraph>Body</Paragraph></Section>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(output.jsx, "<section><h2>Title</h2><p>Body</p></section>");
    }

    #[test]
    fn dynamic_heading_level_uses_the_typed_fallback_component() {
        let source = r#"
            import { Heading } from '@hozo/core'
            const el = <Heading level={level}>Title</Heading>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert_eq!(output.jsx, "<Heading level={level}>Title</Heading>");
    }

    #[test]
    fn article_and_navigation_lower_to_web_landmarks() {
        let source = r#"
            import { Article, Nav, Heading } from '@hozo/core'
            const el = <Article><Heading>Title</Heading><Nav accessibilityLabel="Primary" /></Article>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert_eq!(output.jsx, "<article><h1>Title</h1><nav aria-label={\"Primary\"}></nav></article>");
    }

    #[test]
    fn a_synthesized_interactive_element_gets_keyboard_activation() {
        // A `<div role="button">` receives no Enter or Space from the
        // browser. Emitting `tabIndex` without them produced a control a
        // keyboard user could reach and not operate -- WCAG 2.1.1.
        let source = r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable accessibilityRole="button" onPress={save}>Save</Pressable>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.contains("{...hozoInteractive(save)}"), "{}", output.jsx);
        assert_eq!(output.runtime_imports, KEY_ACTIVATION_IMPORTS);
        // Not separately: the helper supplies it, together with everything
        // it has to agree with.
        assert!(!output.jsx.contains("tabIndex"), "{}", output.jsx);
    }

    #[test]
    fn a_disabled_control_is_inoperable_not_merely_announced() {
        // The bug this whole line of work started from: `aria-disabled`
        // went out and the handler ran anyway. Once keyboard activation
        // existed it ran on Enter and Space as well.
        let source = r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable disabled={busy} accessibilityRole="button" onPress={save}>S</Pressable>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.contains("{...hozoInteractive(save, busy)}"), "{}", output.jsx);
        // One statement of the state, not two that can disagree.
        assert_eq!(output.jsx.matches("aria-disabled").count(), 0, "{}", output.jsx);
    }

    #[test]
    fn both_spellings_of_disabled_fold_into_one_guard() {
        // React Native merges them; emitting both put `aria-disabled` on
        // the element twice and let the later one win by accident.
        let source = r#"
            import { Pressable } from '@hozo/core'
            const el = <Pressable disabled={a} accessibilityState={{ disabled: b }} accessibilityRole="button" onPress={save}>S</Pressable>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(
            output.jsx.contains("{...hozoInteractive(save, (a) || ({ disabled: b }).disabled)}"),
            "{}",
            output.jsx
        );
    }

    #[test]
    fn a_disabled_element_that_is_not_a_control_is_only_announced() {
        // Rule 5: dimming a whole disabled region is a real pattern, and
        // it neither gains nor loses a tab stop.
        let source = r#"
            import { View } from '@hozo/core'
            const el = <View disabled={busy} className="p-4">S</View>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert!(output.jsx.contains("aria-disabled={busy}"), "{}", output.jsx);
        assert!(!output.jsx.contains("hozoInteractive"), "{}", output.jsx);
        assert!(output.runtime_imports.is_empty(), "{}", output.jsx);
    }

    #[test]
    fn an_element_the_browser_already_activates_gets_no_handlers() {
        // `<button>` has Enter and Space natively, and a `<div>` with no
        // press handler is not a control. Adding handlers to either would
        // be runtime nobody asked for.
        for source in [
            r#"
            import { Button } from '@hozo/core'
            const el = <Button onPress={save}>Save</Button>
            "#,
            r#"
            import { View } from '@hozo/core'
            const el = <View className="p-4">Text</View>
            "#,
        ] {
            let parsed = hozo_parser::parse_tsx(source);
            let output = lower(&parsed.roots[0].node, source, &Theme::default());
            assert!(!output.jsx.contains("onKeyDown"), "{}", output.jsx);
            assert!(output.runtime_imports.is_empty(), "{}", output.jsx);
        }
    }

    #[test]
    fn static_and_dynamic_lists_keep_ordering_semantics() {
        let source = r#"
            import { List, ListItem } from '@hozo/core'
            const el = <List ordered><ListItem>First</ListItem></List>
            "#;
        let parsed = hozo_parser::parse_tsx(source);
        let output = lower(&parsed.roots[0].node, source, &Theme::default());
        assert_eq!(output.jsx, "<ol><li>First</li></ol>");

        let dynamic = r#"
            import { List, ListItem } from '@hozo/core'
            const el = <List ordered={ranked}><ListItem>First</ListItem></List>
            "#;
        let parsed = hozo_parser::parse_tsx(dynamic);
        let output = lower(&parsed.roots[0].node, dynamic, &Theme::default());
        assert_eq!(output.jsx, "<List ordered={ranked}><li>First</li></List>");
    }
}

#[cfg(test)]
mod role_tests {
    use super::*;

    fn lower_source(source: &str) -> LowerOutput {
        let parsed = hozo_parser::parse_tsx(source);
        lower(&parsed.roots[0].node, source, &hozo_ir::Theme::default())
    }

    #[test]
    fn an_authored_role_wins_over_the_primitives_own() {
        // A menu built on a list. Announcing both roles announces neither.
        let output = lower_source(
            "import { List } from '@hozo/core'\nexport const C = () => <List role=\"menu\">x</List>\n",
        );
        assert!(output.jsx.contains(r#"<ul role="menu">"#), "{}", output.jsx);
    }

    #[test]
    fn a_role_the_element_already_has_is_kept() {
        // `<ul role="list">` looks redundant and is a documented
        // workaround: Safari drops list semantics from a `<ul>` styled
        // `list-style: none`, and the explicit role is what restores them.
        // Deciding it was redundant would break that.
        let output = lower_source(
            "import { List } from '@hozo/core'\nexport const C = () => <List role=\"list\">x</List>\n",
        );
        assert!(output.jsx.contains(r#"<ul role="list">"#), "{}", output.jsx);
    }

    #[test]
    fn react_natives_spelling_is_normalised_to_aria() {
        // `header` is React Native's word; ARIA's is `heading`. The two
        // vocabularies overlap for most names and not this one.
        let output = lower_source(
            "import { View } from '@hozo/core'\nexport const C = () => <View accessibilityRole=\"header\">x</View>\n",
        );
        assert!(output.jsx.contains(r#"role="heading""#), "{}", output.jsx);
        assert!(!output.jsx.contains("accessibilityRole"), "{}", output.jsx);
    }

    #[test]
    fn a_role_only_react_native_has_is_dropped_and_named() {
        // Guessing the nearest ARIA role would announce something the
        // author never wrote.
        let output = lower_source(
            "import { View } from '@hozo/core'\nexport const C = () => <View accessibilityRole=\"drawerlayout\">x</View>\n",
        );
        assert!(!output.jsx.contains("drawerlayout"), "{}", output.jsx);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|d| d.code == hozo_ir::DiagnosticCode::RoleHasNoWebEquivalent),
            "{:?}",
            output.diagnostics
        );
    }

    #[test]
    fn a_role_aria_does_not_define_is_not_invented() {
        // The vocabulary is a closed list, and a name outside it is not a
        // role -- on either platform.
        let output = lower_source(
            "import { View } from '@hozo/core'\nexport const C = () => <View role=\"widget\">x</View>\n",
        );
        assert!(!output.jsx.contains(r#"role="widget""#), "{}", output.jsx);
    }
}
