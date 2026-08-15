// Prints the conformance report: per-group coverage and fidelity, plus
// every individual mismatch. `pnpm --filter @dowel/tailwind-conformance report`

import { CANDIDATE_GROUPS, ALL_CANDIDATES, stripVariant } from './candidates.ts'
import { loadFullCatalog, namespaceOf } from './catalog.ts'
import { compareCandidate, type Comparison, type Verdict } from './compare.ts'
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
    `\nRestricted (${restricted.length}) -- counted as covered, but only where listed;` +
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

// ---------------------------------------------------------------------------
// Full catalogue
// ---------------------------------------------------------------------------
//
// The number above is measured against a list we chose. This one is
// measured against Tailwind's own, asked for through the same entry point
// its IntelliSense extension uses -- so it can't be flattered by picking
// the utilities Dowel happens to implement.

console.log('\n\n== Full catalogue (Tailwind enumerates its own surface) ==')

const catalog = (await loadFullCatalog()).filter((c) => !c.includes('"'))
const fullOracle = await buildOracle(catalog)
const fullVars = new Map([...loadThemeVars(), ...fullOracle.registerDefaults])

interface NamespaceRow {
  total: number
  match: number
  mismatch: number
  unsupported: number
  skipped: number
}

const catalogCounts: Record<Verdict, number> = {
  MATCH: 0,
  MISMATCH: 0,
  UNSUPPORTED: 0,
  SKIPPED: 0,
  COMPOSITION_ONLY: 0,
}
const byNamespace = new Map<string, NamespaceRow>()
// Entries Tailwind lists but produces no standalone CSS for -- a gradient
// stop with no gradient, a negative form of something that takes no
// negative. There is nothing for Dowel to cover, so they leave the
// denominator. Tailwind decides this, not us.
//
// `COMPOSITION_ONLY` is the same situation one step later: Tailwind emits a
// rule, but it only sets a custom property and paints nothing (`ring-blue-500`
// is the colour a ring renders in, inert until a `ring-2` exists to paint).
// A one-utility comparison can't measure those either, so they leave too.
let notEmittedByTailwind = 0

for (const candidate of catalog) {
  const expected = fullOracle.rules.get(candidate)
  if (expected === undefined) {
    notEmittedByTailwind += 1
    continue
  }
  const result = compareCandidate(candidate, expected, fullVars)
  catalogCounts[result.verdict] += 1
  const ns = namespaceOf(candidate)
  const row =
    byNamespace.get(ns) ?? { total: 0, match: 0, mismatch: 0, unsupported: 0, skipped: 0 }
  row.total += 1
  if (result.verdict === 'MATCH') row.match += 1
  if (result.verdict === 'MISMATCH') row.mismatch += 1
  if (result.verdict === 'UNSUPPORTED') row.unsupported += 1
  if (result.verdict === 'SKIPPED') row.skipped += 1
  byNamespace.set(ns, row)
}

const catalogComparable =
  catalog.length - notEmittedByTailwind - catalogCounts.COMPOSITION_ONLY
console.log(
  `Catalogue:   ${catalog.length} entries. ${notEmittedByTailwind} produce no rule at all from ` +
    `Tailwind and\n             ${catalogCounts.COMPOSITION_ONLY} produce one that paints nothing ` +
    `until combined with another utility;\n             neither is measurable one utility at a ` +
    `time, leaving ${catalogComparable}.\n`,
)
console.log(
  `Match:       ${catalogCounts.MATCH}/${catalogComparable} = ${pct(catalogCounts.MATCH, catalogComparable)}\n` +
    `Mismatch:    ${catalogCounts.MISMATCH}\n` +
    `Unsupported: ${catalogCounts.UNSUPPORTED}   (Dowel emits nothing)\n` +
    `Skipped:     ${catalogCounts.SKIPPED}   (one side wouldn't resolve; no claim made)`,
)
console.log(
  '\nRead the percentage with the shape in mind: value expansion dominates it.\n' +
    'Covering bg-blue-500 and bg-blue-600 is one code path, and mask-* alone is\n' +
    'over a quarter of the catalogue. The namespace table is the actionable view.',
)

const nsRows = [...byNamespace.entries()].sort((a, b) => b[1].total - a[1].total)
console.log('\nLargest namespaces:')
console.table(
  nsRows.slice(0, 20).map(([namespace, row]) => ({
    namespace,
    total: row.total,
    match: row.match,
    mismatch: row.mismatch,
    unsupported: row.unsupported,
    skipped: row.skipped,
  })),
)
const untouched = nsRows.filter(([, r]) => r.match === 0)
console.log(
  `Namespaces: ${nsRows.length} total, ` +
    `${nsRows.filter(([, r]) => r.match === r.total).length} fully matching, ` +
    `${untouched.length} with nothing matching.`,
)
