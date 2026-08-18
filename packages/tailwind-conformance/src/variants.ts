// A denominator for variants, including stacked ones -- and the only
// section that compares *where* a rule applies rather than only what it
// sets.
//
// Two gaps meet here, and they hid each other. Tailwind's `getClassList()`
// enumerates utilities and not the variants in front of them, so no
// derived denominator contains `md:hover:flex`. And every comparison in
// this package matches declaration text, so a rule that lost its `@media`
// wrapper reads as identical to one that kept it. Between the two,
// `md:hover:bg-blue-500` compiled to nothing at all -- for a class people
// write every day -- and nothing in this repository could have said so.
//
// So this section builds the combinations itself and checks three things:
// the declarations, the selector the rule matches, and the at-rules around
// it. The last two are the ones nothing else looks at.

import { compile as hozoCompile } from '@hozo/compiler'
import { extractRules } from './extract.ts'
import { normalize } from './normalize.ts'
import { buildOracle } from './oracle.ts'
import { loadThemeVars } from './theme.ts'

/** The variants Hozo claims to know, plus a few it doesn't. */
const VARIANTS = [
  'hover',
  'focus',
  'disabled',
  'dark',
  'first',
  'sm',
  'md',
  'lg',
  'xl',
  '2xl',
  // Not implemented, and included on purpose: an unsupported variant
  // should read as an honest gap here rather than being absent from the
  // list that decides what "supported" means.
  'last',
  'active',
  'focus-visible',
]

/**
 * Utilities to put the variants in front of.
 *
 * Three, deliberately: the variant is what is under test, and crossing
 * every variant pair with a large sample would measure the utilities
 * again at hundreds of times the cost.
 */
const UTILITIES = ['flex', 'bg-blue-500', 'p-4']

export interface VariantCase {
  candidate: string
  /** Rules Tailwind produces, as (at-rules, selector suffix, declarations). */
  expected: RuleShape[]
}

export interface VariantCatalog {
  cases: VariantCase[]
  /** Theme values plus Tailwind's `@property` register defaults. */
  vars: Map<string, string>
}

export interface RuleShape {
  atRules: string[]
  /** The selector with the class name itself removed: `:hover:first-child`. */
  suffix: string
  declarations: string
}

export interface VariantVerdict {
  candidate: string
  verdict: 'MATCH' | 'MISMATCH' | 'UNSUPPORTED'
  detail?: string
}

/** Every single and stacked pair, kept if Tailwind generates a rule. */
export async function buildVariantCatalog(): Promise<VariantCatalog> {
  const candidates: string[] = []
  for (const utility of UTILITIES) {
    for (const one of VARIANTS) {
      candidates.push(`${one}:${utility}`)
      for (const two of VARIANTS) {
        if (one === two) continue
        candidates.push(`${one}:${two}:${utility}`)
      }
    }
  }

  const oracle = await buildOracle(candidates)
  const cases: VariantCase[] = []
  for (const candidate of candidates) {
    const shapes = shapesFor(oracle.css, escapeForSelector(candidate))
    if (shapes.length > 0) cases.push({ candidate, expected: shapes })
  }
  return { cases, vars: new Map([...loadThemeVars(), ...oracle.registerDefaults]) }
}

export function compareVariant(entry: VariantCase, vars: Map<string, string>): VariantVerdict {
  const source =
    `import { View } from '@hozo/core'\n` +
    `const el = <View className="${entry.candidate}" />\n`
  const [compiled] = hozoCompile(source)
  const actual = compiled ? shapesFor(compiled.css, /\.hozo-\d+/.source, true) : []
  if (actual.length === 0) {
    return { candidate: entry.candidate, verdict: 'UNSUPPORTED' }
  }

  const differences: string[] = []
  for (const [index, want] of entry.expected.entries()) {
    const got = actual[index]
    if (!got) {
      differences.push(`missing rule ${index + 1}`)
      continue
    }
    if (want.suffix !== got.suffix) {
      differences.push(`selector: expected \`&${want.suffix}\`, got \`&${got.suffix}\``)
    }
    const wantAt = want.atRules.map(canonicalAtRule).join(' ')
    const gotAt = got.atRules.map(canonicalAtRule).join(' ')
    if (wantAt !== gotAt) {
      differences.push(`at-rules: expected \`${wantAt || '(none)'}\`, got \`${gotAt || '(none)'}\``)
    }
    const wantDecl = declarationText(want.declarations, vars)
    const gotDecl = declarationText(got.declarations, vars)
    if (wantDecl !== gotDecl) {
      differences.push(`declarations: expected \`${wantDecl}\`, got \`${gotDecl}\``)
    }
  }
  if (actual.length > entry.expected.length) {
    differences.push(`${actual.length - entry.expected.length} rule(s) more than Tailwind emits`)
  }
  return differences.length === 0
    ? { candidate: entry.candidate, verdict: 'MATCH' }
    : { candidate: entry.candidate, verdict: 'MISMATCH', detail: differences.join('; ') }
}

/**
 * `@media (width >= 48rem)` and `@media (min-width: 768px)` are the same
 * query written two ways -- Tailwind's range syntax and Hozo's.
 *
 * Rewritten rather than accepted as a difference: it is a spelling, and
 * leaving it in place would drown every real one.
 */
function canonicalAtRule(rule: string): string {
  return rule
    .replace(/\(width >= ([\d.]+)rem\)/g, (_, rem: string) => `(min-width: ${parseFloat(rem) * 16}px)`)
    .replace(/\s+/g, ' ')
    .trim()
}

/**
 * A rule's declarations, resolved and sorted.
 *
 * Through the shared normalizer rather than a local split, so a theme
 * token resolves the same way it does everywhere else -- comparing the raw
 * text put `var(--color-blue-500)` against the `oklch()` Hozo writes and
 * called every coloured utility a mismatch.
 */
function declarationText(declarations: string, vars: Map<string, string>): string {
  const { declarations: resolved } = normalize(declarations, vars)
  return [...resolved]
    .map(([property, value]) => `${property}: ${value}`)
    .sort()
    .join('; ')
}

/**
 * Every rule targeting `className`, as shapes.
 *
 * The `@layer` wrapper is dropped: both sides put their utilities in one,
 * and it says nothing about where the rule applies.
 */
function shapesFor(css: string, className: string, isPattern = false): RuleShape[] {
  const target = isPattern ? new RegExp(className) : new RegExp(`\\.${className}(?![\\w-])`)
  const shapes: RuleShape[] = []
  for (const rule of extractRules(css)) {
    if (!target.test(rule.selector)) continue
    // The base `.hozo-view` rule is View's own semantics, not this
    // candidate's.
    if (rule.selector === '.hozo-view') continue
    shapes.push({
      atRules: rule.atRules.filter((at) => !at.startsWith('@layer')),
      suffix: rule.selector.replace(target, '&').replace(/^&/, ''),
      declarations: rule.declarations,
    })
  }
  return shapes
}

/** How Tailwind escapes a candidate when writing it as a CSS selector. */
function escapeForSelector(candidate: string): string {
  return candidate.replace(/[^\w-]/g, (ch) => (ch.charCodeAt(0) > 127 ? ch : `\\\\${ch}`))
}
