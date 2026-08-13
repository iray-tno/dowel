//! TSX analysis and Style IR construction.

mod dynamic_class;
mod jsx;
mod tailwind;

use dowel_ir::{Diagnostic, Node};
use jsx::JsxCollector;
use oxc_allocator::Allocator;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_span::SourceType;

pub struct ParseOutput {
    pub roots: Vec<Node>,
    /// Diagnostics about the source as written, independent of target
    /// platform -- backends raise their own separately during `lower()`.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses TSX source into Dowel IR node trees, one per top-level JSX
/// element found (e.g. one per component's returned JSX).
pub fn parse_tsx(source_text: &str) -> ParseOutput {
    let allocator = Allocator::default();
    let source_type = SourceType::from_extension("tsx").expect("\"tsx\" is a known extension");
    let ret = Parser::new(&allocator, source_text, source_type).parse();

    let mut collector = JsxCollector::new(&ret.module_record);
    collector.visit_program(&ret.program);

    ParseOutput { roots: collector.roots, diagnostics: collector.diagnostics }
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

        let root = &output.roots[0];
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
}
