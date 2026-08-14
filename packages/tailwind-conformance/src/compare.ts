// Compares one utility's Dowel output against the Tailwind oracle's,
// producing an explicit verdict per candidate. The verdicts distinguish
// three failure modes that mean very different things:
//
//   UNSUPPORTED -- Dowel emits nothing. A coverage gap, not a bug.
//   MISMATCH    -- both emit, and they disagree. A fidelity bug.
//   SKIPPED     -- the normalizer couldn't resolve one side confidently,
//                  so no claim is made either way.

import { compile as dowelCompile } from '@dowel/compiler'
import { extractRules } from './extract.ts'
import { normalize } from './normalize.ts'

export type Verdict = 'MATCH' | 'MISMATCH' | 'UNSUPPORTED' | 'SKIPPED'

export interface Comparison {
  candidate: string
  verdict: Verdict
  detail?: string
}

/**
 * Differences that are deliberate and permanent, so they shouldn't sit in
 * the report as standing mismatches. Each needs a reason, not just an
 * entry -- an allowlist is the easiest place to hide a real bug.
 */
const ACCEPTED_DIFFERENCES: Record<string, { property: string; reason: string }> = {
  'rounded-full': {
    property: 'border-radius',
    // Tailwind emits `calc(infinity * 1px)`. React Native has no infinity,
    // and a large finite radius is already clamped to 50% of the box by
    // both platforms, so the rendering is identical for any real element.
    reason: 'infinity has no React Native equivalent; 9999px is the conventional stand-in',
  },
}

/** Runs a single utility through Dowel and returns its declaration block. */
function dowelDeclarations(candidate: string): string {
  const source = `import { View } from '@dowel/core'\nconst el = <View className="${candidate}" />\n`
  const results = dowelCompile(source)
  if (results.length === 0) return ''
  return extractRules(results[0].css)
    // Drop the shared `.dowel-view` base rule -- it's View's own semantics
    // (proposal 8.1), not anything this utility produced.
    .filter((rule) => rule.selector !== '.dowel-view')
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

  const dowelBlock = dowelDeclarations(candidate)
  if (dowelBlock.trim() === '') {
    return { candidate, verdict: 'UNSUPPORTED' }
  }

  const expected = normalize(oracleBlock, vars)
  const actual = normalize(dowelBlock, vars)

  if (expected.unresolved.length > 0) {
    return {
      candidate,
      verdict: 'SKIPPED',
      detail: `unresolvable in tailwind output: ${expected.unresolved.join(', ')}`,
    }
  }
  if (actual.unresolved.length > 0) {
    return {
      candidate,
      verdict: 'SKIPPED',
      detail: `unresolvable in dowel output: ${actual.unresolved.join(', ')}`,
    }
  }
  if (expected.declarations.size === 0) {
    return { candidate, verdict: 'SKIPPED', detail: 'tailwind rule had no comparable declarations' }
  }

  const accepted = ACCEPTED_DIFFERENCES[candidate]
  const detail = diffSummary(expected.declarations, actual.declarations, accepted)
  if (detail !== '') return { candidate, verdict: 'MISMATCH', detail }
  return accepted
    ? { candidate, verdict: 'MATCH', detail: `accepted difference: ${accepted.reason}` }
    : { candidate, verdict: 'MATCH' }
}
