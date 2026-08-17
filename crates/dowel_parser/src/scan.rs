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
//!
//! With one subtraction: ranges the compiler *did* read exactly are
//! skipped. Without that, `className="p-4"` would both compile into a
//! scoped rule and reappear as `.p-4` in the candidate stylesheet -- which
//! would grow with the app's entire static utility surface and give back
//! the bloat precise reading exists to avoid. The subtraction is
//! per-occurrence, not global: the same class written statically in one
//! file and produced dynamically in another is still covered by the
//! second file's scan.

use dowel_ir::{Condition, SourceSpan, StyleProperty};

use crate::tailwind;

/// A candidate class name that resolves to real style properties.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedUtility {
    /// The class exactly as written, e.g. `hover:bg-blue-500`. Emitted as
    /// the CSS selector so a runtime-produced string matches it.
    pub class_name: String,
    /// The condition/properties groups the class produces.
    ///
    /// A list rather than one pair because `container` is six of them --
    /// see `tailwind::expand_class`. Everything else has exactly one.
    pub groups: Vec<(Condition, Vec<StyleProperty>)>,
}

/// Bytes that can appear inside a Tailwind class. Anything else ends a
/// candidate.
///
/// Byte-wise rather than char-wise so token boundaries are also byte
/// offsets, which is what the consumed-span subtraction compares against.
/// Safe for UTF-8: every class byte is ASCII, so a multi-byte character's
/// continuation bytes always end a token rather than splitting one.
fn is_class_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b':' | b'/' | b'.' | b'[' | b']' | b'%' | b'!')
}

/// Resolves one class name, or `None` if it isn't a utility Dowel knows.
pub fn resolve_class_name(class_name: &str) -> Option<ScannedUtility> {
    let groups: Vec<_> = tailwind::expand_class(class_name)
        .into_iter()
        .filter(|(_, properties)| !properties.is_empty())
        .collect();
    if groups.is_empty() {
        return None;
    }
    Some(ScannedUtility { class_name: class_name.to_string(), groups })
}

/// Class names in `source` that resolve to real styles and that the
/// compiler did *not* already read exactly.
///
/// Returns names rather than resolved utilities because that's what the
/// build cache stores: a name is a durable fact about the source, while
/// the properties behind it are derived from Dowel's utility table and
/// would go stale the moment that table changed.
///
/// Deduplicated, in first-appearance order.
pub fn scan_class_candidates(source: &str) -> Vec<String> {
    // A source that doesn't parse yields no consumed spans, so it degrades
    // to a plain scan -- more candidates than necessary, never fewer.
    let consumed = crate::parse_tsx(source).consumed_class_spans;
    scan_outside(source, &consumed)
}

fn is_consumed(consumed: &[SourceSpan], start: usize, end: usize) -> bool {
    consumed.iter().any(|span| start < span.end as usize && end > span.start as usize)
}

fn scan_outside(source: &str, consumed: &[SourceSpan]) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut found: Vec<String> = Vec::new();
    let mut start: Option<usize> = None;

    for i in 0..=bytes.len() {
        if i < bytes.len() && is_class_byte(bytes[i]) {
            start.get_or_insert(i);
            continue;
        }
        let Some(token_start) = start.take() else { continue };
        if is_consumed(consumed, token_start, i) {
            continue;
        }
        let token = &source[token_start..i];
        if found.iter().any(|name| name == token) {
            continue;
        }
        if resolve_class_name(token).is_some() {
            found.push(token.to_string());
        }
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
        let names = scan_class_candidates(source);
        assert!(names.contains(&"p-4".to_string()));
        assert!(names.contains(&"p-8".to_string()));
    }

    #[test]
    fn keeps_variant_prefixes_intact() {
        let found = scan_class_candidates("const c = 'hover:bg-blue-500'");
        assert_eq!(found, vec!["hover:bg-blue-500"]);
        assert_eq!(resolve_class_name("hover:bg-blue-500").unwrap().groups[0].0, Condition::Hover);
    }

    #[test]
    fn ignores_tokens_that_are_not_utilities() {
        // Ordinary identifiers and paths shouldn't produce rules.
        let found = scan_class_candidates("import { useState } from 'react'");
        assert!(found.is_empty(), "unexpected: {found:?}");
    }

    #[test]
    fn deduplicates() {
        let found = scan_class_candidates("const a = 'p-4', b = 'p-4', c = 'p-4'");
        assert_eq!(found, vec!["p-4"]);
    }

    #[test]
    fn skips_classes_the_compiler_already_compiled_away() {
        // The whole point of reading `className` precisely: `p-4` becomes
        // a scoped rule, so shipping `.p-4` as well would put the app's
        // entire static utility surface back into the bundle.
        let source = r#"
            import { View } from '@dowel/core'
            const el = <View className="p-4 text-xl" />
        "#;
        assert!(scan_class_candidates(source).is_empty(), "{:?}", scan_class_candidates(source));
    }

    #[test]
    fn keeps_the_unreadable_half_of_a_mixed_class_name() {
        // `p-4` compiled away; `bg-blue-500` only exists as a call's
        // return value, so it still needs a rule under its own name.
        let source = r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            function accent() { return 'bg-blue-500' }
            const el = <View className={cn('p-4', accent())} />
        "#;
        assert_eq!(scan_class_candidates(source), vec!["bg-blue-500"]);
    }

    #[test]
    fn a_class_compiled_away_here_is_still_found_where_it_is_dynamic() {
        // The subtraction is per-occurrence. Writing `p-4` statically in
        // one place must not hide the copy another module produces.
        let source = r#"
            import { View } from '@dowel/core'
            function pick() { return 'p-4' }
            const a = <View className="p-4" />
            const b = <View className={pick()} />
        "#;
        assert_eq!(scan_class_candidates(source), vec!["p-4"]);
    }
}
