//! TSX analysis and Style IR construction.

mod arbitrary;
pub mod aria;
mod aria_check;
mod dynamic_class;
mod jsx;
mod scan;
mod tailwind;
mod tailwind_variants;

pub use jsx::is_primitive_name;
pub use scan::{resolve_class_name, scan_class_candidates, ScannedUtility};

use hozo_ir::Diagnostic;
use jsx::JsxCollector;
pub use jsx::Root;
use oxc_allocator::Allocator;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;

pub struct ParseOutput {
    pub roots: Vec<Root>,
    /// Diagnostics about the source as written, independent of target
    /// platform -- backends raise their own separately during `lower()`.
    pub diagnostics: Vec<Diagnostic>,
    /// Source ranges the compiler read exactly and turned into scoped
    /// rules. The candidate scan subtracts these, so a class that already
    /// compiled away doesn't also ship under its Tailwind name.
    pub consumed_class_spans: Vec<hozo_ir::SourceSpan>,
}

/// Every binding a source file imports from one module, by local name.
///
/// Narrower than it looks: the Native backend needs it to avoid
/// re-declaring a binding the file already imported from `react-native`,
/// which is a SyntaxError rather than a duplicate.
pub fn module_imports(source_text: &str, module: &str) -> Vec<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_extension("tsx").expect("\"tsx\" is a known extension");
    let ret = Parser::new(&allocator, source_text, source_type).parse();

    ret.module_record
        .import_entries
        .iter()
        .filter(|entry| !entry.is_type && entry.module_request.name.as_str() == module)
        .map(|entry| entry.local_name.name.to_string())
        .collect()
}

/// Names a *trusted* module exports that are not the primitive Hozo means.
///
/// One entry, and it matters: React Native's `Button` takes a `title` and
/// renders no children, while Hozo's is a semantic primitive that lowers to
/// a Pressable wrapping its children. Trusting `react-native` wholesale
/// turned an ordinary `<Button title="Go" onPress={f} />` into
/// `<Pressable accessibilityRole="button" onPress={f} title="Go"></Pressable>`
/// -- a control that renders nothing at all, in a file the author never
/// asked Hozo to change.
///
/// Every other name React Native and Hozo share -- `View`, `Text`,
/// `Image`, `ScrollView`, `FlatList`, `Pressable`, `TextInput` -- is the
/// same component, which is why the rest of `react-native` is trusted.
const INCOMPATIBLE_PRIMITIVES: &[(&str, &str)] = &[("react-native", "Button")];

/// Primitive-named bindings that must not be lowered: imported from a
/// module the project doesn't trust, or named on the incompatibility list.
pub fn foreign_primitives(
    source_text: &str,
    sources: &[String],
) -> std::collections::HashSet<String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_extension("tsx").expect("\"tsx\" is a known extension");
    let ret = Parser::new(&allocator, source_text, source_type).parse();

    ret.module_record
        .import_entries
        .iter()
        .filter(|entry| !entry.is_type)
        .filter(|entry| jsx::is_primitive_name(entry.local_name.name.as_str()))
        .filter(|entry| {
            let module = entry.module_request.name.as_str();
            let local = entry.local_name.name.as_str();
            !sources.iter().any(|s| s == module)
                || INCOMPATIBLE_PRIMITIVES.contains(&(module, local))
        })
        .map(|entry| entry.local_name.name.to_string())
        .collect()
}

/// Parses TSX source into Hozo IR node trees, one per top-level JSX
/// element found (e.g. one per component's returned JSX).
pub fn parse_tsx(source_text: &str) -> ParseOutput {
    parse_tsx_with(source_text, None)
}

/// Parses TSX, lowering only primitives imported from `sources`.
///
/// `None` trusts every module, which is what a caller with no project
/// configuration to consult wants -- and what `parse_tsx` has always done.
///
/// The list is per *tag*, not per file. A real Expo app has
/// `<View className="p-4">` from `react-native` and `<Button label="Save">`
/// from `@expo/ui` in the same tree, and those names are the same names:
/// `@expo/ui` exports `Text`, `Button`, `List`, `ListItem`, `ScrollView`
/// and `TextInput`, every one a native platform component sharing nothing
/// with the Hozo primitive but its spelling. Refusing the whole file would
/// leave the half Hozo does understand uncompiled; lowering the whole file
/// would replace someone's SwiftUI button with a `<div>`. So a foreign tag
/// becomes `Child::Verbatim` -- carried, exactly like any other component
/// the compiler does not model -- and the tree around it compiles.
pub fn parse_tsx_with(source_text: &str, sources: Option<&[String]>) -> ParseOutput {
    let allocator = Allocator::default();
    let source_type = SourceType::from_extension("tsx").expect("\"tsx\" is a known extension");
    let ret = Parser::new(&allocator, source_text, source_type).parse();

    let foreign = match sources {
        None => std::collections::HashSet::new(),
        Some(sources) => foreign_primitives(source_text, sources),
    };
    let scope = jsx::Scope { module_record: &ret.module_record, foreign };

    let mut collector = JsxCollector::new(&scope);
    collector.visit_program(&ret.program);

    for root in &collector.roots {
        aria_check::check(&root.node, &mut collector.diagnostics);
    }

    ParseOutput {
        roots: collector.roots,
        diagnostics: collector.diagnostics,
        consumed_class_spans: collector.consumed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hozo_ir::{Child, Primitive};

    /// Unwraps a child that should be a Hozo primitive.
    fn node(child: &Child) -> &hozo_ir::Node {
        match child {
            Child::Node(node) => node,
            other => panic!("expected a primitive, got {other:?}"),
        }
    }

    #[test]
    fn a_component_the_compiler_does_not_model_is_a_boundary_not_a_wall() {
        // `<Card><View/></Card>` compiled to nothing at all. The outermost
        // element is someone else's component, so no root was collected --
        // and the walk stopped there rather than looking inside, so every
        // primitive under any wrapper the author wrote fell back to the
        // runtime components with no diagnostic. Passing children into
        // your own component is ordinary React.
        let source = r#"
            import { View, Text } from '@hozo/core'
            import { Card, Panel } from 'some-lib'
            const el = (
              <Card>
                <Panel><View className="p-4"><Text>deep</Text></View></Panel>
                <View className="p-8">sibling</View>
              </Card>
            )
            "#;
        let parsed = parse_tsx(source);
        // One root per primitive found, however deep the wrappers went.
        assert_eq!(parsed.roots.len(), 2);
        assert_eq!(parsed.roots[0].node.primitive, Primitive::View);
        assert_eq!(parsed.roots[1].node.primitive, Primitive::View);
        // And the tree under each is still built, not flattened.
        assert_eq!(node(&parsed.roots[0].node.children[0]).primitive, Primitive::Text);
    }

    #[test]
    fn a_primitive_under_a_known_root_is_not_collected_twice() {
        // The other half of the same branch: when a node *was* built, its
        // children came with it, so walking again would collect them a
        // second time and splice the same span twice.
        let source = r#"
            import { View, Text } from '@hozo/core'
            const el = <View className="p-4"><Text>once</Text></View>
            "#;
        let parsed = parse_tsx(source);
        assert_eq!(parsed.roots.len(), 1);
        assert_eq!(node(&parsed.roots[0].node.children[0]).primitive, Primitive::Text);
    }

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
    fn parses_login_example_into_a_node_tree() {
        let output = parse_tsx(LOGIN_EXAMPLE);
        assert_eq!(output.roots.len(), 1);

        let root = &output.roots[0].node;
        assert_eq!(root.primitive, Primitive::View);
        assert_eq!(root.children.len(), 2);
        assert!(!root.style.is_empty());

        let text = node(&root.children[0]);
        assert_eq!(text.primitive, Primitive::Text);
        assert_eq!(text.children, vec![Child::Text("Welcome".to_string())]);

        let button = node(&root.children[1]);
        assert_eq!(button.primitive, Primitive::Button);
        assert_eq!(button.children, vec![Child::Text("Continue".to_string())]);
    }

    #[test]
    fn children_the_compiler_does_not_model_are_carried_not_dropped() {
        // Until 2026-08-15 every one of these vanished from the output with
        // no diagnostic: `children` could only hold Hozo primitives, so
        // anything else had nowhere to go.
        let source = r#"
            import { View, Text } from '@hozo/core'
            export function C({ show, items, name }) {
              return (
                <View>
                  <Avatar />
                  {show && <Text>hi</Text>}
                  {items.map((i) => <Text key={i}>{i}</Text>)}
                  <Text>Hello {name}</Text>
                </View>
              )
            }
            "#;
        let output = parse_tsx(source);
        let root = &output.roots[0].node;

        let verbatim: Vec<&str> = root
            .children
            .iter()
            .filter_map(|child| match child {
                Child::Verbatim { source: r, .. } => Some(&source[r.0.start as usize..r.0.end as usize]),
                _ => None,
            })
            .collect();
        assert_eq!(verbatim.len(), 3, "{:?}", root.children);
        assert_eq!(verbatim[0], "<Avatar />");
        assert!(verbatim[1].starts_with("{show &&"), "{}", verbatim[1]);
        assert!(verbatim[2].starts_with("{items.map"), "{}", verbatim[2]);

        // Mixed text and expression keeps both, in order -- `<Text>Hello
        // {name}</Text>` is not `<Text>{name} Hello</Text>`.
        let last = node(root.children.last().unwrap());
        assert_eq!(last.children.len(), 2);
        // The trailing space is significant -- `Hello {name}` is not
        // `Hello{name}`. Only whitespace containing a newline is dropped.
        assert_eq!(last.children[0], Child::Text("Hello ".to_string()));
        assert!(matches!(last.children[1], Child::Verbatim { .. }));
    }

    #[test]
    fn jsx_whitespace_rules_are_followed_not_trimmed() {
        // Whitespace containing a newline goes (that is what makes
        // indented markup work); whitespace inside a line stays (that is
        // what keeps `Hello {name}` from becoming `Hello{name}`).
        let source = r#"
            import { Text } from '@hozo/core'
            export function C({ name }) {
              return (
                <Text>
                  Hello {name}, welcome
                </Text>
              )
            }
            "#;
        let root = &parse_tsx(source).roots[0].node;
        assert_eq!(root.children[0], Child::Text("Hello ".to_string()));
        assert_eq!(root.children[2], Child::Text(", welcome".to_string()));
    }

    #[test]
    fn whitespace_between_tags_is_not_a_child() {
        // JSX collapses it away, so recording it would give every indented
        // element empty text siblings.
        let output = parse_tsx(LOGIN_EXAMPLE);
        assert_eq!(output.roots[0].node.children.len(), 2);
    }

    /// The slot is where a generated `const x = useSomething()` can be
    /// spliced. A statement is the only safe position: calling a hook
    /// inline in the JSX breaks the rules of hooks the moment the element
    /// sits behind a conditional.
    #[test]
    fn a_function_component_offers_a_hook_slot_just_inside_its_brace() {
        let output = parse_tsx(LOGIN_EXAMPLE);
        let slot = output.roots[0].hook_slot.expect("Login() has a block body");
        assert_eq!(&LOGIN_EXAMPLE[slot as usize - 1..slot as usize], "{");
    }

    #[test]
    fn a_block_bodied_arrow_offers_one_too() {
        let source = "import { View } from '@hozo/core'\n\
                      export const Card = () => { return <View /> }\n";
        let output = parse_tsx(source);
        let slot = output.roots[0].hook_slot.expect("block-bodied arrow");
        assert_eq!(&source[slot as usize - 1..slot as usize], "{");
    }

    #[test]
    fn jsx_with_nowhere_to_put_a_statement_has_no_slot() {
        // A concise arrow body is an expression, and module scope has no
        // enclosing function at all. Neither can hold a hook declaration,
        // so conditions that need one must be refused rather than compiled
        // into something invalid.
        for source in [
            "import { View } from '@hozo/core'\nexport const Card = () => <View />\n",
            "import { View } from '@hozo/core'\nconst el = <View />\n",
        ] {
            let output = parse_tsx(source);
            assert_eq!(output.roots[0].hook_slot, None, "{source}");
        }
    }

    #[test]
    fn a_nested_function_shadows_its_parent() {
        // The hook belongs to the function that actually renders the JSX,
        // not to whatever encloses it.
        let source = "import { View } from '@hozo/core'\n\
                      export function Outer() {\n\
                      \x20 function Inner() { return <View /> }\n\
                      \x20 return Inner\n\
                      }\n";
        let output = parse_tsx(source);
        let slot = output.roots[0].hook_slot.expect("Inner has a block body");
        let inner_brace = source.find("Inner() {").unwrap() + "Inner() {".len();
        assert_eq!(slot as usize, inner_brace);
    }
}

#[cfg(test)]
mod import_tests {
    use super::*;

    fn trusted() -> Vec<String> {
        vec!["react-native".to_string(), "@hozo/core".to_string()]
    }

    #[test]
    fn a_renamed_import_is_judged_by_the_name_the_jsx_uses() {
        // `View as Box` makes `<Box>`, which the tag matcher declines
        // anyway -- so the local name is what matters, not the exported
        // one. The reverse is the dangerous direction:
        // `Pressable as View` from a module nobody trusts puts a foreign
        // component behind a name Hozo would otherwise lower.
        assert!(foreign_primitives("import { View as Box } from 'some-ui-kit'
", &trusted()).is_empty());

        let foreign =
            foreign_primitives("import { Pressable as View } from 'some-ui-kit'
", &trusted());
        assert!(foreign.contains("View"));
    }

    #[test]
    fn a_type_only_import_binds_nothing_at_runtime() {
        // Nothing a JSX tag can resolve to, so nothing to decline.
        assert!(foreign_primitives("import type { View } from 'some-ui-kit'
", &trusted()).is_empty());
        assert!(foreign_primitives("import { type View } from 'some-ui-kit'
", &trusted()).is_empty());
    }

    #[test]
    fn ordinary_imports_are_not_primitives() {
        assert!(foreign_primitives("import { useState } from 'react'
", &trusted()).is_empty());
    }
}

#[cfg(test)]
mod incompatibility_tests {
    use super::*;

    #[test]
    fn react_natives_button_is_not_hozos() {
        // Trusting `react-native` wholesale turned an ordinary
        // `<Button title="Go" onPress={f} />` into
        // `<Pressable onPress={f} title="Go"></Pressable>` -- a control
        // that renders nothing at all, in a file nobody asked Hozo to
        // change. The two components share a name and no API.
        let sources = vec!["react-native".to_string(), "@hozo/core".to_string()];
        let foreign = foreign_primitives("import { Button, View } from 'react-native'\n", &sources);
        assert!(foreign.contains("Button"));
        assert!(!foreign.contains("View"), "the rest of react-native is the same component");
    }

    #[test]
    fn hozos_own_button_is_lowered() {
        let sources = vec!["react-native".to_string(), "@hozo/core".to_string()];
        let foreign = foreign_primitives("import { Button } from '@hozo/core'\n", &sources);
        assert!(foreign.is_empty());
    }

    #[test]
    fn an_untrusted_module_is_foreign_whatever_it_exports() {
        let sources = vec!["react-native".to_string()];
        let foreign = foreign_primitives("import { Text, Button } from '@expo/ui/swift-ui'\n", &sources);
        assert!(foreign.contains("Text"));
        assert!(foreign.contains("Button"));
    }
}

#[cfg(test)]
mod aria_pattern_tests {
    use super::*;

    fn diagnostics_for(jsx: &str) -> Vec<String> {
        let source = format!(
            "import {{ View }} from '@hozo/core'\nexport const C = () => ({jsx})\n"
        );
        parse_tsx(&source).diagnostics.into_iter().map(|d| d.message).collect()
    }

    #[test]
    fn a_role_missing_what_it_requires_is_reported() {
        let messages = diagnostics_for(r#"<View role="combobox">x</View>"#);
        assert!(messages.iter().any(|m| m.contains("aria-expanded")), "{messages:?}");
    }

    #[test]
    fn a_complete_pattern_says_nothing() {
        assert!(diagnostics_for(
            r#"<View role="combobox" aria-expanded={open} aria-controls="list">x</View>"#
        )
        .is_empty());
    }

    #[test]
    fn accessibility_state_could_be_supplying_it() {
        // One opaque expression whose keys are never read -- the Web
        // backend emits `({expr}).expanded` and lets the value decide. So
        // it may or may not carry the state, and "cannot tell" is the
        // honest answer rather than a warning the author cannot act on.
        assert!(diagnostics_for(
            r#"<View role="combobox" aria-controls="l" accessibilityState={{ expanded: open }}>x</View>"#
        )
        .is_empty());
    }

    #[test]
    fn a_role_outside_its_container_is_reported() {
        let messages = diagnostics_for(r#"<View role="tab">x</View>"#);
        assert!(messages.iter().any(|m| m.contains("tablist")), "{messages:?}");
        assert!(diagnostics_for(
            r#"<View role="tablist"><View role="tab">x</View></View>"#
        )
        .is_empty());
    }

    #[test]
    fn a_container_without_what_it_must_hold_is_reported() {
        let messages = diagnostics_for(r#"<View role="listbox"><View>x</View></View>"#);
        assert!(messages.iter().any(|m| m.contains("option")), "{messages:?}");
    }

    #[test]
    fn an_unreadable_child_makes_the_answer_unknowable() {
        // `{items.map(render)}` may produce a hundred options or none.
        // Warning here would be a warning nobody can act on, and the rest
        // of the compiler already treats a carried expression this way.
        assert!(diagnostics_for(r#"<View role="listbox">{items.map(render)}</View>"#).is_empty());
    }

    #[test]
    fn a_spread_could_be_supplying_it() {
        assert!(diagnostics_for(r#"<View role="combobox" {...rest}>x</View>"#).is_empty());
    }
}
