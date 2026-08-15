//! TSX analysis and Style IR construction.

mod dynamic_class;
mod jsx;
mod scan;
mod tailwind;

pub use scan::{resolve_class_name, scan_class_candidates, ScannedUtility};

use dowel_ir::Diagnostic;
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
    pub consumed_class_spans: Vec<dowel_ir::SourceSpan>,
}

/// Parses TSX source into Dowel IR node trees, one per top-level JSX
/// element found (e.g. one per component's returned JSX).
pub fn parse_tsx(source_text: &str) -> ParseOutput {
    let allocator = Allocator::default();
    let source_type = SourceType::from_extension("tsx").expect("\"tsx\" is a known extension");
    let ret = Parser::new(&allocator, source_text, source_type).parse();

    let mut collector = JsxCollector::new(&ret.module_record);
    collector.visit_program(&ret.program);

    ParseOutput {
        roots: collector.roots,
        diagnostics: collector.diagnostics,
        consumed_class_spans: collector.consumed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dowel_ir::{Primitive, TextContent};

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
    fn parses_login_example_into_a_node_tree() {
        let output = parse_tsx(LOGIN_EXAMPLE);
        assert_eq!(output.roots.len(), 1);

        let root = &output.roots[0].node;
        assert_eq!(root.primitive, Primitive::View);
        assert_eq!(root.children.len(), 2);
        assert!(!root.style.is_empty());

        let text = &root.children[0];
        assert_eq!(text.primitive, Primitive::Text);
        assert_eq!(text.text, Some(TextContent::Literal("Welcome".to_string())));

        let button = &root.children[1];
        assert_eq!(button.primitive, Primitive::Button);
        assert_eq!(button.text, Some(TextContent::Literal("Continue".to_string())));
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
        let source = "import { View } from '@dowel/core'\n\
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
            "import { View } from '@dowel/core'\nexport const Card = () => <View />\n",
            "import { View } from '@dowel/core'\nconst el = <View />\n",
        ] {
            let output = parse_tsx(source);
            assert_eq!(output.roots[0].hook_slot, None, "{source}");
        }
    }

    #[test]
    fn a_nested_function_shadows_its_parent() {
        // The hook belongs to the function that actually renders the JSX,
        // not to whatever encloses it.
        let source = "import { View } from '@dowel/core'\n\
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
