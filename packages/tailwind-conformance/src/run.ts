// Prints the conformance report: per-group coverage and fidelity, plus
// every individual mismatch. `pnpm --filter @dowel/tailwind-conformance report`

import { CANDIDATE_GROUPS, ALL_CANDIDATES, stripVariant } from './candidates.ts'
import { compareCandidate, type Comparison } from './compare.ts'
import { compareNativeCandidate, type NativeComparison, type NativeVerdict } from './native.ts'
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

console.log(`Tailwind conformance vs tailwindcss v${tailwindVersion()}\n`)
console.log('== Web (dowel_web) ==')
// Stated up front so the Web numbers can't be read as covering both
// backends: Tailwind only exists as CSS, so it can only be an oracle for
// the Web lowering. The Native section below measures coverage only.

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

// ---------------------------------------------------------------------------
// Native
// ---------------------------------------------------------------------------

console.log('\n\n== Native (dowel_native) ==')
console.log('Coverage only: Tailwind is CSS, so there is no oracle to check fidelity against.')
console.log('REFUSED is a known gap named at build time; SILENT is one nothing reports.\n')

const nativeResults = new Map<string, NativeComparison[]>()
for (const [group, candidates] of Object.entries(CANDIDATE_GROUPS)) {
  nativeResults.set(group, candidates.map(compareNativeCandidate))
}
const nativeAll = [...nativeResults.values()].flat()
const nativeCount = (verdict: NativeVerdict, list: NativeComparison[] = nativeAll) =>
  list.filter((r) => r.verdict === verdict).length

console.table(
  [...nativeResults.entries()].map(([group, list]) => ({
    group,
    total: list.length,
    covered: nativeCount('COVERED', list),
    restricted: list.filter((r) => r.restrictedTo).length,
    refused: nativeCount('REFUSED', list),
    silent: nativeCount('SILENT', list),
  })),
)

console.log(
  `Coverage:    ${nativeCount('COVERED')}/${nativeAll.length} = ` +
    `${pct(nativeCount('COVERED'), nativeAll.length)}  (Dowel emits a style)`,
)
console.log(
  `Refused:     ${nativeCount('REFUSED')}   (named at build time; error, or warning where the ` +
    `gap is unbuilt rather than impossible)\n` +
    `Silent:      ${nativeCount('SILENT')}   (no style, no diagnostic)`,
)

const restricted = nativeAll.filter((r) => r.restrictedTo)
if (restricted.length > 0) {
  console.log(
    `\nRestricted (${restricted.length}) -- counted as covered, but only on some primitives;` +
      `\nusing them elsewhere is a build error:`,
  )
  for (const r of restricted) {
    console.log(`  ${r.candidate}: ${r.restrictedTo!.join(', ')} only`)
  }
}

const refusedByGroup = [...nativeResults.entries()]
  .map(([group, list]) => [group, list.filter((r) => r.verdict === 'REFUSED')] as const)
  .filter(([, list]) => list.length > 0)
if (refusedByGroup.length > 0) {
  console.log('\nRefused, by group:')
  for (const [group, list] of refusedByGroup) {
    console.log(`  ${group}: ${list.map((r) => r.candidate).join(' ')}`)
  }
}

const silent = nativeAll.filter((r) => r.verdict === 'SILENT')
if (silent.length > 0) {
  console.log(`\nSilent (${silent.length}) -- these compile to nothing without saying so:`)
  for (const s of silent) {
    console.log(`  ${s.candidate}: ${s.detail}`)
  }
}
