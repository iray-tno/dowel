// Compares one utility's Hozo output against the Tailwind oracle's,
// producing an explicit verdict per candidate. The verdicts distinguish
// three failure modes that mean very different things:
//
//   UNSUPPORTED      -- Hozo emits nothing. A coverage gap, not a bug.
//   MISMATCH         -- both emit, and they disagree. A fidelity bug.
//   SKIPPED          -- the normalizer couldn't resolve one side
//                       confidently, so no claim is made either way.
//   COMPOSITION_ONLY -- Tailwind itself paints nothing standalone, so a
//                       one-utility comparison can't measure it at all.
//                       Not a gap: emitting nothing *is* the right answer
//                       here, and it leaves the denominator for the same
//                       reason the candidates Tailwind emits no rule for do.

import { compile as hozoCompile } from '@hozo/compiler'
import { extractRules } from './extract.ts'
import { normalize } from './normalize.ts'

export type Verdict = 'MATCH' | 'MISMATCH' | 'UNSUPPORTED' | 'SKIPPED' | 'COMPOSITION_ONLY'

export interface Comparison {
  candidate: string
  verdict: Verdict
  detail?: string
}

/**
 * Differences that are deliberate and permanent, so they shouldn't sit in
 * the report as standing mismatches. Each needs a reason, not just an
 * entry -- an allowlist is the easiest place to hide a real bug.
 *
 * Deliberately empty: the one entry that lived here (`rounded-full`) was
 * justified by a React Native limitation, which is not a valid excuse in a
 * comparison that only ever exercises the *Web* backend. Hozo now models
 * the radius as an intent (`Radius::Full`) so Web emits Tailwind's exact
 * `calc(infinity * 1px)` and only Native falls back to a finite value.
 */
const ACCEPTED_DIFFERENCES: Record<string, { property: string; reason: string }> = {
  'bg-linear-to-r from-red-500 from-blue-500 to-green-500': {
    property: 'background-image',
    reason:
      'Two utilities setting the same thing, resolved by different orders. Hozo compiles a ' +
      'class attribute into one rule, so the later class wins -- blue. Tailwind emits a rule ' +
      'per class and lets the cascade decide, so whichever it happens to write later wins -- ' +
      'red, on nothing the author can see. Matching it would mean reproducing Tailwind\'s ' +
      'internal utility sort, and the answer it gives is the one people file bugs about.',
  },
  'shadow-none': {
    property: 'box-shadow',
    reason:
      'Hozo emits `none`; Tailwind clears its own `--tw-shadow` register and leaves the ' +
      'ring/inset chain in place, which resolves to fully transparent layers. Both render no ' +
      'shadow. The composition hazard flagged when this was added is now handled: `shadow-none` ' +
      'clears only the shadow layer, so `shadow-none ring-2` still draws the ring.',
  },
}

/** Runs a single utility through Hozo and returns its declaration block. */
function hozoDeclarations(candidate: string): string {
  const source = `import { View } from '@hozo/core'\nconst el = <View className="${candidate}" />\n`
  const results = hozoCompile(source)
  if (results.length === 0) return ''
  const rules = extractRules(results[0].css)
    // Keep only rules targeting one of Hozo's generated classes. That
    // drops the shared `.hozo-view` base rule (View's own semantics,
    // proposal 8.1, not something this utility produced) and the steps
    // inside an `@keyframes` block, whose selectors are `to`/`50%` and
    // whose declarations would otherwise be counted as the utility's own.
    .filter((rule) => rule.selector.includes('.hozo-') && rule.selector !== '.hozo-view')
  if (rules.length === 0) return ''

  // Only the least-conditional rules, which is what the expected side
  // holds too: a nested `@media` inside Tailwind's rule is dropped by
  // `extractRules` because it describes a different element state.
  //
  // Symmetry, not a shortcut. `container` is the only utility that emits
  // both -- `width: 100%` plus a max-width at each breakpoint -- and
  // counting Hozo's conditional half against an expected that has none
  // reported `extra max-width: 1536px`, a difference that exists only
  // between the two sides of this function. A responsive candidate like
  // `md:flex-row` is unaffected: every rule it produces is at the same
  // depth, so the minimum keeps all of them.
  const depth = Math.min(...rules.map((rule) => rule.atRules.length))
  return rules
    .filter((rule) => rule.atRules.length === depth)
    .map((rule) => rule.declarations)
    .join('')
}

function diffSummary(
  expected: Map<string, string>,
  actual: Map<string, string>,
  accepted?: { property: string },
): string {
  const parts: string[] = []
  for (const [prop, value] of expected) {
    if (accepted && accepted.property === prop) continue
    const got = actual.get(prop)
    if (got === undefined) parts.push(`missing ${prop}: ${value}`)
    else if (got !== value) parts.push(`${prop}: expected ${value}, got ${got}`)
  }
  for (const [prop, value] of actual) {
    if (accepted && accepted.property === prop) continue
    if (!expected.has(prop)) parts.push(`extra ${prop}: ${value}`)
  }
  return parts.join('; ')
}

export function compareCandidate(
  candidate: string,
  oracleBlock: string | undefined,
  vars: Map<string, string>,
): Comparison {
  if (!oracleBlock) {
    return { candidate, verdict: 'SKIPPED', detail: 'tailwind produced no rule for this candidate' }
  }

  const expected = normalize(oracleBlock, vars)
  if (expected.unresolved.length > 0) {
    return {
      candidate,
      verdict: 'SKIPPED',
      detail: `unresolvable in tailwind output: ${expected.unresolved.join(', ')}`,
    }
  }
  // Checked before Hozo's side, deliberately. Some utilities only set a
  // custom property and paint nothing on their own -- `ring-blue-500` is
  // the colour a ring renders in, and does nothing until a `ring-2` exists
  // to paint. A standalone comparison cannot measure those either way, so
  // scoring them as a Hozo gap would be wrong: emitting nothing is the
  // correct output. Same treatment as the candidates Tailwind produces no
  // rule for at all -- out of the denominator, decided by Tailwind.
  if (expected.declarations.size === 0) {
    return {
      candidate,
      verdict: 'COMPOSITION_ONLY',
      detail: 'tailwind paints nothing standalone; only meaningful combined with another utility',
    }
  }

  const hozoBlock = hozoDeclarations(candidate)
  if (hozoBlock.trim() === '') {
    return { candidate, verdict: 'UNSUPPORTED' }
  }

  const actual = normalize(hozoBlock, vars)
  if (actual.unresolved.length > 0) {
    return {
      candidate,
      verdict: 'SKIPPED',
      detail: `unresolvable in hozo output: ${actual.unresolved.join(', ')}`,
    }
  }

  const accepted = ACCEPTED_DIFFERENCES[candidate]
  const detail = diffSummary(expected.declarations, actual.declarations, accepted)
  if (detail !== '') return { candidate, verdict: 'MISMATCH', detail }
  return accepted
    ? { candidate, verdict: 'MATCH', detail: `accepted difference: ${accepted.reason}` }
    : { candidate, verdict: 'MATCH' }
}
