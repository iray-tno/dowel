//! Finds Tailwind-class-looking strings anywhere in a source file.
//!
//! This is deliberately *imprecise*, and that's the point. Everywhere else
//! Dowel reads `className` exactly, from the JSX AST -- which is why it can
//! compile those away completely. The cost of that precision is that it
//! can't see a class it never reads, so `className={getDynamic()}` has no
//! CSS behind it (proposal §7's third tier).
//!
//! Scanning is what closes that gap, and it's how Tailwind itself works:
//! its `oxide` crate byte-scans source files for candidate strings rather
//! than understanding the code. A candidate found here isn't known to be
//! used -- `getDynamic()` might never return it -- so this only feeds the
//! fallback path, never the precise one. False positives cost unused CSS
//! rules; a missed candidate costs a silently unstyled element, so the
//! scan errs toward including too much.

use dowel_ir::{Condition, StyleProperty};

use crate::tailwind;

/// A candidate class name that resolves to real style properties.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedUtility {
    /// The class exactly as written, e.g. `hover:bg-blue-500`. Emitted as
    /// the CSS selector so a runtime-produced string matches it.
    pub class_name: String,
    pub condition: Condition,
    pub properties: Vec<StyleProperty>,
}

/// Characters that can appear inside a Tailwind class. Anything else ends
/// a candidate.
fn is_class_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '/' | '.' | '[' | ']' | '%' | '!')
}

/// Splits `source` into candidate tokens and keeps the ones that resolve.
///
/// Deduplicated, in first-appearance order so output stays deterministic.
pub fn scan_class_candidates(source: &str) -> Vec<ScannedUtility> {
    let mut found: Vec<ScannedUtility> = Vec::new();
    for token in source.split(|c: char| !is_class_char(c)) {
        if token.is_empty() || found.iter().any(|u| u.class_name == token) {
            continue;
        }
        let (condition, properties) = tailwind::expand_utility(token);
        if properties.is_empty() {
            continue;
        }
        found.push(ScannedUtility {
            class_name: token.to_string(),
            condition,
            properties,
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_classes_the_ast_never_sees() {
        // The whole reason this exists: `p-4` here is inside a function
        // body, not a className the parser reads.
        let source = r#"
            function getDynamic() {
              return isWide ? 'p-4' : 'p-8'
            }
        "#;
        let names: Vec<_> = scan_class_candidates(source).into_iter().map(|u| u.class_name).collect();
        assert!(names.contains(&"p-4".to_string()));
        assert!(names.contains(&"p-8".to_string()));
    }

    #[test]
    fn keeps_variant_prefixes_intact() {
        let found = scan_class_candidates("const c = 'hover:bg-blue-500'");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].class_name, "hover:bg-blue-500");
        assert_eq!(found[0].condition, Condition::Hover);
    }

    #[test]
    fn ignores_tokens_that_are_not_utilities() {
        // Ordinary identifiers and paths shouldn't produce rules.
        let found = scan_class_candidates("import { useState } from 'react'");
        assert!(found.is_empty(), "unexpected: {found:?}");
    }

    #[test]
    fn deduplicates() {
        let found = scan_class_candidates("'p-4' 'p-4' 'p-4'");
        assert_eq!(found.len(), 1);
    }
}
