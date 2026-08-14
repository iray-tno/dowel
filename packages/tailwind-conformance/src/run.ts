// Prints the conformance report: per-group coverage and fidelity, plus
// every individual mismatch. `pnpm --filter @dowel/tailwind-conformance report`

import { CANDIDATE_GROUPS, ALL_CANDIDATES, stripVariant } from './candidates.ts'
import { compareCandidate, type Comparison } from './compare.ts'
import { buildOracle } from './oracle.ts'
import { loadThemeVars, tailwindVersion } from './theme.ts'

const vars = loadThemeVars()
const oracle = await buildOracle(ALL_CANDIDATES)

const results = new Map<string, Comparison[]>()
for (const [group, candidates] of Object.entries(CANDIDATE_GROUPS)) {
  results.set(
    group,
    candidates.map((candidate) => compareCandidate(candidate, oracle.get(candidate), vars)),
  )
}

const all = [...results.values()].flat()
const count = (verdict: string, list: Comparison[] = all) => list.filter((r) => r.verdict === verdict).length
const pct = (n: number, d: number) => (d === 0 ? '--' : `${((n / d) * 100).toFixed(1)}%`)

console.log(`Tailwind conformance vs tailwindcss v${tailwindVersion()}\n`)

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
