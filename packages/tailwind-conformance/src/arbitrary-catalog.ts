// A denominator for Tailwind's arbitrary syntax, derived rather than
// chosen.
//
// The named catalogue could be enumerated because Tailwind's own
// `getClassList()` enumerates it. Arbitrary values can't be: there are
// infinitely many, which is exactly why the 100% Web figure never covered
// any of them -- the whole syntax sat outside the measurement, and the one
// bug class it hid (a font size compiled as a colour) was invisible for
// precisely that reason.
//
// So the list is built by asking Tailwind, not by picking favourites. Every
// prefix that appears anywhere in the real catalogue is crossed with a
// fixed set of representative values, and whatever Tailwind emits a rule
// for is what Hozo is then held to. Neither half is ours to choose: the
// prefixes come from Tailwind's class list and the verdict comes from
// Tailwind's compiler.

import { loadFullCatalog } from './catalog.ts'
import { buildOracle, type Oracle } from './oracle.ts'

/**
 * One value per CSS data type an arbitrary value can carry.
 *
 * Representative rather than exhaustive, and each is here for a reason:
 * the units differ in whether React Native can resolve them, `calc()` and
 * `var()` are the shapes no design system can precompute, `#ff0000` is the
 * colour form that made `text-[...]` ambiguous in the first place, and
 * `nonsense` is included because Tailwind accepts it -- it validates
 * arbitrary values not at all, so a compiler that refused would generate
 * less than the engine it is measured against.
 */
const VALUES = [
  '10px',
  '2rem',
  '1.5em',
  '50%',
  '#ff0000',
  'rgb(0_0_0)',
  'calc(100%-2rem)',
  'var(--x)',
  '1.5',
  'auto',
  '100vh',
  'nonsense',
]

/**
 * Every dash-prefix that appears in the real catalogue.
 *
 * `bg-blue-500` yields `bg` and `bg-blue`; only one of those turns out to
 * accept an arbitrary value, and the oracle is what decides which. Casting
 * wide and letting Tailwind reject is the point -- a hand-written prefix
 * list would quietly omit whatever we forgot, and the report would look
 * complete.
 */
export function prefixesFrom(catalog: string[]): string[] {
  const prefixes = new Set<string>()
  for (const name of catalog) {
    const parts = name.replace(/^-/, '').split('-')
    for (let take = 1; take < parts.length; take += 1) {
      prefixes.add(parts.slice(0, take).join('-'))
    }
    // A whole name is a prefix too: `shadow` takes `shadow-[0_0_4px_red]`
    // and is also a complete utility on its own.
    prefixes.add(parts.join('-'))
  }
  return [...prefixes]
}

/**
 * The candidates Tailwind actually generates a rule for.
 *
 * Everything else is dropped from the denominator rather than counted as a
 * Hozo gap -- the same treatment the named catalogue gives a candidate
 * Tailwind emits nothing for. A class the reference engine ignores is not
 * a coverage failure, and counting it as one would make the score
 * unimprovable and therefore useless.
 */
export async function buildArbitraryCatalog(): Promise<{
  candidates: string[]
  oracle: Oracle
}> {
  const catalog = await loadFullCatalog()
  const probes: string[] = []
  for (const prefix of prefixesFrom(catalog)) {
    for (const value of VALUES) {
      probes.push(`${prefix}-[${value}]`)
    }
  }
  // The whole-class forms and the `(--var)` shorthand, which have no
  // prefix to cross with anything.
  probes.push('[color:red]', '[--my-var:4px]', '[mask-type:luminance]', '[display:grid]')
  for (const prefix of ['bg', 'text', 'border', 'w', 'h', 'p', 'm', 'fill', 'shadow']) {
    probes.push(`${prefix}-(--brand)`)
  }
  // Arbitrary variants, whose lowering is a different mechanism again --
  // one rewrites a selector, one wraps the rule in an at-rule.
  for (const variant of ['[&>*]', '[&_a]', '[&:nth-child(3)]', '[.dark_&]', '[@media(print)]', '[@supports(display:grid)]']) {
    probes.push(`${variant}:p-4`, `${variant}:text-red-500`)
  }

  const oracle = await buildOracle(probes)
  return { candidates: probes.filter((c) => oracle.rules.has(c)), oracle }
}
