//! Decomposes dynamic `className` expressions (proposal §7).
//!
//! The real dividing line isn't "how complex is the condition" -- it's
//! whether the set of possible output strings is enumerable at compile
//! time. A condition is never evaluated or interpreted here, only
//! delimited: it's captured as an opaque `ExprRef` and re-emitted verbatim
//! by a later lowering stage. Anything that isn't one of the recognized
//! shapes below (a literal, a `&&`-guard, a ternary, or a call to a
//! verified `cn`/`clsx`/`classnames` import) becomes a fallback leaf rather
//! than failing the whole node -- fallback is per-leaf, not per-node.

use dowel_ir::{Condition, ConditionExpr, ExprRef, SourceSpan, StyleDeclaration};
use oxc_ast::ast::{
    Argument, CallExpression, ConditionalExpression, Expression, IdentifierReference,
    JSXExpression, LogicalExpression, LogicalOperator, StringLiteral,
};
use oxc_span::{GetSpan, Span};
use oxc_syntax::module_record::ModuleRecord;

use crate::tailwind;

/// Import sources whose default export/named exports are trusted to behave
/// like `clsx` (join truthy string arguments with a space) when recognized
/// as a call. Not scope-aware: a *local* binding that happens to share the
/// same name as an import elsewhere in the file (shadowing) is not
/// detected, since this checks the module's top-level import table rather
/// than doing full scope resolution. Rare in practice; falls back safely if
/// the name isn't imported from one of these packages at all.
const RECOGNIZED_CX_MODULES: [&str; 3] = ["clsx", "classnames", "tailwind-merge"];

#[derive(Default)]
pub struct Decomposed {
    pub declarations: Vec<StyleDeclaration>,
    pub fallback: Vec<ExprRef>,
}

pub fn decompose_class_name(expr: &JSXExpression, module_record: &ModuleRecord) -> Decomposed {
    let mut out = Decomposed::default();
    decompose(classify_jsx_expression(expr), None, &mut out, module_record);
    out
}

/// The handful of expression shapes `decompose` understands, regardless of
/// which wrapper enum (`Expression`, `Argument`, `JSXExpression`) they were
/// extracted from -- those wrappers differ only at the outermost level;
/// everything nested below is a plain `Expression`.
enum Target<'a, 'b> {
    StringLiteral(&'b StringLiteral<'a>),
    Logical(&'b LogicalExpression<'a>),
    Conditional(&'b ConditionalExpression<'a>),
    Call(&'b CallExpression<'a>),
    Spread(Span),
    Other(Span),
}

fn classify_expression<'a, 'b>(expr: &'b Expression<'a>) -> Target<'a, 'b> {
    match expr {
        Expression::StringLiteral(lit) => Target::StringLiteral(lit),
        Expression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
            Target::Logical(logical)
        }
        Expression::ConditionalExpression(cond) => Target::Conditional(cond),
        Expression::CallExpression(call) => Target::Call(call),
        other => Target::Other(other.span()),
    }
}

fn classify_argument<'a, 'b>(arg: &'b Argument<'a>) -> Target<'a, 'b> {
    match arg {
        Argument::StringLiteral(lit) => Target::StringLiteral(lit),
        Argument::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
            Target::Logical(logical)
        }
        Argument::ConditionalExpression(cond) => Target::Conditional(cond),
        Argument::CallExpression(call) => Target::Call(call),
        Argument::SpreadElement(spread) => Target::Spread(spread.span()),
        other => Target::Other(other.span()),
    }
}

fn classify_jsx_expression<'a, 'b>(expr: &'b JSXExpression<'a>) -> Target<'a, 'b> {
    match expr {
        JSXExpression::StringLiteral(lit) => Target::StringLiteral(lit),
        JSXExpression::LogicalExpression(logical) if logical.operator == LogicalOperator::And => {
            Target::Logical(logical)
        }
        JSXExpression::ConditionalExpression(cond) => Target::Conditional(cond),
        JSXExpression::CallExpression(call) => Target::Call(call),
        other => Target::Other(other.span()),
    }
}

fn to_expr_ref(span: Span) -> ExprRef {
    ExprRef(SourceSpan { start: span.start, end: span.end })
}

fn and_ref(current: &Option<ConditionExpr>, guard: Span) -> Option<ConditionExpr> {
    let guard = ConditionExpr::Ref(to_expr_ref(guard));
    match current {
        None => Some(guard),
        Some(existing) => Some(ConditionExpr::And(Box::new(existing.clone()), Box::new(guard))),
    }
}

fn and_not_ref(current: &Option<ConditionExpr>, guard: Span) -> Option<ConditionExpr> {
    let negated = ConditionExpr::Not(Box::new(ConditionExpr::Ref(to_expr_ref(guard))));
    match current {
        None => Some(negated),
        Some(existing) => Some(ConditionExpr::And(Box::new(existing.clone()), Box::new(negated))),
    }
}

fn to_condition(expr: Option<ConditionExpr>) -> Condition {
    match expr {
        None => Condition::Always,
        Some(expr) => Condition::Expr(expr),
    }
}

fn is_recognized_cx_call(callee: &Expression, module_record: &ModuleRecord) -> Option<()> {
    let Expression::Identifier(ident) = callee else { return None };
    is_recognized_cx_identifier(ident, module_record)
}

fn is_recognized_cx_identifier(
    ident: &IdentifierReference,
    module_record: &ModuleRecord,
) -> Option<()> {
    let name = ident.name.as_str();
    module_record.import_entries.iter().find_map(|entry| {
        (entry.local_name.name.as_str() == name
            && RECOGNIZED_CX_MODULES.contains(&entry.module_request.name.as_str()))
        .then_some(())
    })
}

fn decompose(
    target: Target,
    condition: Option<ConditionExpr>,
    out: &mut Decomposed,
    module_record: &ModuleRecord,
) {
    match target {
        Target::StringLiteral(lit) => {
            for token in lit.value.split_whitespace() {
                for property in tailwind::expand_utility(token) {
                    out.declarations
                        .push(StyleDeclaration { property, condition: to_condition(condition.clone()) });
                }
            }
        }
        Target::Logical(logical) => {
            let guarded = and_ref(&condition, logical.left.span());
            decompose(classify_expression(&logical.right), guarded, out, module_record);
        }
        Target::Conditional(cond) => {
            let when_true = and_ref(&condition, cond.test.span());
            decompose(classify_expression(&cond.consequent), when_true, out, module_record);
            let when_false = and_not_ref(&condition, cond.test.span());
            decompose(classify_expression(&cond.alternate), when_false, out, module_record);
        }
        Target::Call(call) if is_recognized_cx_call(&call.callee, module_record).is_some() => {
            for arg in &call.arguments {
                decompose(classify_argument(arg), condition.clone(), out, module_record);
            }
        }
        Target::Call(call) => {
            // Unrecognized callee: opaque leaf, same as `Other` below.
            out.fallback.push(to_expr_ref(call.span));
        }
        Target::Spread(span) | Target::Other(span) => {
            // Opaque leaf: a spread argument or any other expression shape.
            // Falls back regardless of `condition` -- if this leaf is
            // itself already inside a recognized guard, the guard was
            // applied to the *literals* it selects between, not to whether
            // the leaf needs runtime evaluation at all.
            out.fallback.push(to_expr_ref(span));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dowel_ir::{Color, FlexShorthand, StyleProperty};
    use oxc_allocator::Allocator;
    use oxc_ast::ast::JSXAttributeValue;
    use oxc_ast_visit::Visit;
    use oxc_parser::Parser;
    use oxc_span::SourceType;

    /// Finds the first JSX element's `className` `ExpressionContainer` in
    /// `source` and runs it through `decompose_class_name`.
    fn decompose_source(source: &str) -> Decomposed {
        let allocator = Allocator::default();
        let source_type = SourceType::from_extension("tsx").unwrap();
        let ret = Parser::new(&allocator, source, source_type).parse();

        struct Finder<'a, 'r> {
            module_record: &'r ModuleRecord<'a>,
            result: Option<Decomposed>,
        }
        impl<'a> Visit<'a> for Finder<'a, '_> {
            fn visit_jsx_attribute(&mut self, it: &oxc_ast::ast::JSXAttribute<'a>) {
                if self.result.is_some() {
                    return;
                }
                if let Some(JSXAttributeValue::ExpressionContainer(container)) = &it.value {
                    self.result = Some(decompose_class_name(&container.expression, self.module_record));
                }
            }
        }
        let mut finder = Finder { module_record: &ret.module_record, result: None };
        finder.visit_program(&ret.program);
        finder.result.expect("no className expression container found in source")
    }

    #[test]
    fn decomposes_logical_and_guard() {
        let out = decompose_source(
            r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn('p-4', active && 'flex-1')} />
            "#,
        );
        assert_eq!(out.fallback.len(), 0);
        // 'p-4' -> 4 padding longhands under Always, 'flex-1' -> 1 declaration under Expr(active)
        assert_eq!(out.declarations.len(), 5);
        let flex_decl =
            out.declarations.iter().find(|d| d.property == StyleProperty::Flex(FlexShorthand::Grow(1.0)));
        assert!(matches!(flex_decl.unwrap().condition, Condition::Expr(ConditionExpr::Ref(_))));
    }

    #[test]
    fn decomposes_ternary_into_true_and_false_branches() {
        let out = decompose_source(
            r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn(size === 'lg' ? 'p-6' : 'p-2')} />
            "#,
        );
        assert_eq!(out.fallback.len(), 0);
        assert_eq!(out.declarations.len(), 8); // 4 padding sides x 2 branches
        let has_not = out
            .declarations
            .iter()
            .any(|d| matches!(&d.condition, Condition::Expr(ConditionExpr::Not(_))));
        assert!(has_not, "the false branch should carry a Not(..) condition");
    }

    #[test]
    fn falls_back_on_unverified_cx_import() {
        let out = decompose_source(
            r#"
            const cn = (a) => a
            import { View } from '@dowel/core'
            const el = <View className={cn('p-4', active && 'flex-1')} />
            "#,
        );
        // `cn` is locally defined here, not imported from clsx/classnames/
        // tailwind-merge, so the whole call must fall back rather than be
        // silently (mis)compiled as if it behaved like clsx.
        assert_eq!(out.declarations.len(), 0);
        assert_eq!(out.fallback.len(), 1);
    }

    #[test]
    fn falls_back_on_opaque_class_name() {
        let out = decompose_source(
            r#"
            import { View } from '@dowel/core'
            const el = <View className={classNameFromProps} />
            "#,
        );
        assert_eq!(out.declarations.len(), 0);
        assert_eq!(out.fallback.len(), 1);
    }

    #[test]
    fn parses_color_token() {
        let out = decompose_source(
            r#"
            import { View } from '@dowel/core'
            import { cn } from 'clsx'
            const el = <View className={cn('bg-blue-500')} />
            "#,
        );
        assert_eq!(
            out.declarations,
            vec![StyleDeclaration {
                property: StyleProperty::BackgroundColor(Color::Token("blue-500".to_string())),
                condition: Condition::Always,
            }]
        );
    }
}
