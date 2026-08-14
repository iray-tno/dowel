// Prints the conformance report: per-group coverage and fidelity, plus
// every individual mismatch. `pnpm --filter @dowel/tailwind-conformance report`

import { CANDIDATE_GROUPS, ALL_CANDIDATES, stripVariant } from './candidates.ts'
import { compareCandidate, type Comparison } from './compare.ts'
import { buildOracle } from './oracle.ts'
import { loadThemeVars, tailwindVersion } from './theme.ts'

const oracle = await buildOracle(ALL_CANDIDATES)
// Theme values plus the `@property` register defaults the utilities
// reference without a fallback -- both needed to resolve Tailwind's output.
const vars = new Map([...loadThemeVars(), ...oracle.registerDefaults])

const results = new Map<string, Comparison[]>()
for (const [group, candidates] of Object.entries(CANDIDATE_GROUPS)) {
  results.set(
    group,
    candidates.map((candidate) => compareCandidate(candidate, oracle.rules.get(candidate), vars)),
  )
}

const all = [...results.values()].flat()
const count = (verdict: string, list: Comparison[] = all) => list.filter((r) => r.verdict === verdict).length
const pct = (n: number, d: number) => (d === 0 ? '--' : `${((n / d) * 100).toFixed(1)}%`)

console.log(`Tailwind conformance vs tailwindcss v${tailwindVersion()}`)
// Stated up front so the headline numbers can't be read as covering both
// backends: Tailwind only exists as CSS, so it can only be an oracle for
// the Web lowering. dowel_native has no external ground truth to compare
// against and is covered by its own assertions instead.
console.log('Scope: Web backend only (dowel_web). Native lowering is not exercised here.\n')

const rows = [...results.entries()].map(([group, list]) => {
  const comparable = list.length - count('SKIPPED', list)
  return {
    group,
    total: list.length,
    supported: `${list.length - count('UNSUPPORTED', list) - count('SKIPPED', list)}/${comparable}`,
    match: count('MATCH', list),
    mismatch: count('MISMATCH', list),
    unsupported: count('UNSUPPORTED', list),
    skipped: count('SKIPPED', list),
  }
})
console.table(rows)

const comparable = all.length - count('SKIPPED')
const supported = count('MATCH') + count('MISMATCH')
console.log(`Candidates:  ${all.length}   (comparable: ${comparable}, skipped: ${count('SKIPPED')})`)
console.log(`Coverage:    ${supported}/${comparable} = ${pct(supported, comparable)}  (Dowel emits something)`)
console.log(`Fidelity:    ${count('MATCH')}/${supported} = ${pct(count('MATCH'), supported)}  (of those, matches Tailwind exactly)`)

const mismatches = all.filter((r) => r.verdict === 'MISMATCH')
if (mismatches.length > 0) {
  console.log(`\nMismatches (${mismatches.length}):`)
  for (const m of mismatches) {
    const { variant } = stripVariant(m.candidate)
    console.log(`  ${m.candidate}${variant ? ` [variant: ${variant}]` : ''}\n    ${m.detail}`)
  }
}

const unsupported = [...results.entries()]
  .map(([group, list]) => [group, list.filter((r) => r.verdict === 'UNSUPPORTED')] as const)
  .filter(([, list]) => list.length > 0)
if (unsupported.length > 0) {
  const total = unsupported.reduce((n, [, list]) => n + list.length, 0)
  console.log(`\nUnsupported (${total}) -- coverage gaps, by group:`)
  for (const [group, list] of unsupported) {
    console.log(`  ${group}: ${list.map((r) => r.candidate).join(' ')}`)
  }
}

const accepted = all.filter((r) => r.verdict === 'MATCH' && r.detail)
if (accepted.length > 0) {
  console.log(`\nAccepted differences (${accepted.length}) -- counted as matches:`)
  for (const a of accepted) {
    console.log(`  ${a.candidate}: ${a.detail}`)
  }
}

const skipped = all.filter((r) => r.verdict === 'SKIPPED')
if (skipped.length > 0) {
  console.log(`\nSkipped (${skipped.length}) -- no claim made either way:`)
  for (const s of skipped) {
    console.log(`  ${s.candidate}: ${s.detail}`)
  }
}
