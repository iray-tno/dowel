// Prints the conformance report: per-group coverage and fidelity, plus
// every individual mismatch. `pnpm --filter @dowel/tailwind-conformance report`

import { auditRefusal, type RefusalAudit, type RefusalVerdict } from './audit.ts'
import { CANDIDATE_GROUPS, ALL_CANDIDATES, stripVariant } from './candidates.ts'
import { buildArbitraryCatalog } from './arbitrary-catalog.ts'
import { buildCompositionCatalog } from './compositions.ts'
import { buildVariantCatalog, compareVariant } from './variants.ts'
import { loadFullCatalog, namespaceOf } from './catalog.ts'
import { reactNativeCssProperties, reactNativeVersion } from './native-surface.ts'
import { compareCandidate, type Comparison, type Verdict } from './compare.ts'
import { compareNativeCandidate, type NativeComparison, type NativeVerdict } from './native.ts'
import { typeCheckStyles } from './typecheck.ts'
import { classesDefinedIn, renderWeb } from './render.ts'
import { compile as dowelCompile, compileNative } from '@dowel/compiler'
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
    noOp: nativeCount('NO_OP', list),
  })),
)

console.log(
  `Coverage:    ${nativeCount('COVERED')}/${nativeAll.length} = ` +
    `${pct(nativeCount('COVERED'), nativeAll.length)}  (Dowel emits a style)`,
)
console.log(
  `Refused:     ${nativeCount('REFUSED')}   (named at build time; error, or warning where the ` +
    `gap is unbuilt rather than impossible)\n` +
    `Silent:      ${nativeCount('SILENT')}   (no style, no diagnostic -- the one that has to stay at zero)\n` +
    `No-op:       ${nativeCount('NO_OP')}   (React Native already does this, so there is nothing to emit)`,
)

const noOps = nativeAll.filter((r) => r.verdict === 'NO_OP')
if (noOps.length > 0) {
  console.log(`\nNo-op (${noOps.length}) -- honoured by doing nothing, not dropped:`)
  for (const n of noOps) {
    console.log(`  ${n.candidate}: ${n.detail}`)
  }
}

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

// ---------------------------------------------------------------------------
// Refusal audit
// ---------------------------------------------------------------------------
//
// The Native coverage number above is a ratio whose denominator Dowel picks:
// a utility Dowel refuses leaves the numerator and the denominator together,
// so a wrong refusal never moves the percentage and nothing points at it.
// This section takes the refusals apart against React Native's own types --
// the same "ask the other tool to enumerate itself" move the full catalogue
// made for the Web side.

console.log('\n\n== Refusal audit (React Native enumerates its own surface) ==')
console.log(
  `Checked against react-native ${reactNativeVersion()}'s StyleSheet types` +
    `${reactNativeCssProperties() ? ', cross-read with react-native-css' : ''}.\n` +
    'SUSPECT does not mean wrong -- the types are a necessary condition, not a\n' +
    'sufficient one. It means the refusal is a claim the types no longer support.\n',
)

const audits = new Map<string, RefusalAudit[]>()
let nativeRefusals = 0
for (const candidate of catalog) {
  const native = compareNativeCandidate(candidate)
  if (native.verdict !== 'REFUSED') continue
  nativeRefusals += 1
  // Only WEB_ONLY refusals assert anything about React Native.
  // VARIANT_NOT_WIRED already says "Dowel hasn't built this", which is not a
  // claim the types can contradict.
  if (native.code !== 'WEB_ONLY_PROPERTY_ON_NATIVE') continue
  const audit = auditRefusal(candidate, fullOracle.rules.get(candidate), fullVars)
  audits.set(audit.verdict, [...(audits.get(audit.verdict) ?? []), audit])
}

const auditCount = (v: RefusalVerdict) => audits.get(v)?.length ?? 0
const checked = auditCount('CONFIRMED') + auditCount('PARTIAL') + auditCount('SUSPECT')
console.log(
  `Refusals:    ${nativeRefusals} on the catalogue, ${checked} checkable against the types\n` +
    `Confirmed:   ${auditCount('CONFIRMED')}   (React Native has no way to hold this)\n` +
    `Partial:     ${auditCount('PARTIAL')}   (some of what it sets is expressible, some isn't)\n` +
    `Suspect:     ${auditCount('SUSPECT')}   (every property it sets is expressible -- re-read these)\n` +
    `Uncheckable: ${auditCount('UNCHECKABLE')}   (Tailwind's own output resolves to no plain declaration)`,
)

// Grouped by the CSS the utilities set, because the suspects come in
// families: one wrong refusal is usually hundreds of candidates, and the
// fix is one code path.
const suspectFamilies = new Map<string, string[]>()
for (const audit of audits.get('SUSPECT') ?? []) {
  const key = audit.expressible.join(', ')
  suspectFamilies.set(key, [...(suspectFamilies.get(key) ?? []), audit.candidate])
}
if (suspectFamilies.size > 0) {
  console.log('\nSuspect refusals, grouped by the CSS they set:')
  for (const [properties, list] of [...suspectFamilies].sort((a, b) => b[1].length - a[1].length)) {
    const shown = list.slice(0, 6).join(' ')
    console.log(
      `  ${properties}  (${list.length})\n    ${shown}${list.length > 6 ? ` ... +${list.length - 6}` : ''}`,
    )
  }
}

const partials = audits.get('PARTIAL') ?? []
if (partials.length > 0) {
  console.log('\nPartial -- refusing the whole utility is defensible, but note what is reachable:')
  for (const audit of partials) {
    console.log(`  ${audit.candidate}: can hold ${audit.expressible.join(', ')}; cannot hold ${audit.inexpressible.join(', ')}`)
  }
}

// ---------------------------------------------------------------------------
// Native output, checked by React Native
// ---------------------------------------------------------------------------
//
// The refusal audit above checks the denominator. This checks the
// numerator, which nothing did until 2026-08-16: for every style Dowel
// actually emits, `tsc` asks React Native's own declarations whether it
// would be accepted. Same check an app would get, rather than this repo's
// regex reading of the same file.

console.log('\n\n== Native output (React Native type-checks what we emit) ==')

const emitted: { candidate: string; style: string }[] = []
for (const candidate of catalog) {
  const expected = fullOracle.rules.get(candidate)
  if (expected === undefined) continue
  if (compareCandidate(candidate, expected, fullVars).verdict !== 'MATCH') continue
  for (const context of ['View', 'Text']) {
    const source =
      `import { ${context} } from '@dowel/core'\n` +
      `export function C() { return <${context} className="${candidate}">x</${context}> }\n`
    const [result] = compileNative(source)
    if (!result) continue
    for (const match of result.styles.matchAll(/^ {2}(\w+): (\{\n[\s\S]*?^ {2}\}),/gm)) {
      const body = match[2].replace(/^ {2}/gm, '')
      if (body.replace(/\s/g, '') !== '{}') emitted.push({ candidate, style: body })
    }
  }
}

const typeErrors = typeCheckStyles(emitted)
console.log(
  `Entries:     ${emitted.length} style objects, from every candidate that matches on Web\n` +
    `Rejected:    ${typeErrors.length}   (by react-native ${reactNativeVersion()}'s own declarations)`,
)
if (typeErrors.length > 0) {
  const byCandidate = new Map<string, string>()
  for (const error of typeErrors) {
    if (!byCandidate.has(error.candidate ?? '?')) {
      byCandidate.set(error.candidate ?? '?', error.message)
    }
  }
  console.log('\nRejected, one line per candidate:')
  for (const [candidate, message] of byCandidate) {
    console.log(`  ${candidate}: ${message}`)
  }
}

// ---------------------------------------------------------------------------
// Web output, rendered
// ---------------------------------------------------------------------------
//
// Everything above compares text. This runs the generated component and
// looks at what comes out -- that the JSX parses, that it mounts, and that
// the class reaching the element is one the stylesheet has a rule for.
// Nothing established any of that until 2026-08-16.

console.log('\n\n== Web output (rendered, not compared as text) ==')

const rendering: { name: string; jsx: string }[] = []
const sheets = new Map<string, { candidate: string; css: string }>()
ALL_CANDIDATES.forEach((candidate, index) => {
  const source =
    `import { View } from '@dowel/core'\n` +
    `export function C() { return <View className="${candidate}">x</View> }\n`
  const [compiled] = dowelCompile(source)
  if (!compiled) return
  const name = `C${index}`
  rendering.push({ name, jsx: compiled.jsx })
  sheets.set(name, { candidate, css: compiled.css })
})

const dangling: string[] = []
for (const output of renderWeb(rendering)) {
  const sheet = sheets.get(output.name)
  if (!sheet) continue
  const defined = classesDefinedIn(sheet.css)
  if ([...output.classes].some((name) => !defined.has(name))) dangling.push(sheet.candidate)
}
console.log(
  `Rendered:    ${rendering.length}/${ALL_CANDIDATES.length} components mounted\n` +
    `Dangling:    ${dangling.length}   (a class reached the DOM with no rule behind it)`,
)
if (dangling.length > 0) {
  console.log(`  ${dangling.join(' ')}`)
}

// == Arbitrary syntax =====================================================
//
// A separate denominator because it has to be built differently: the named
// catalogue is enumerable and this one isn't, so the candidates are derived
// by crossing every prefix Tailwind uses with a set of representative
// values and keeping whatever Tailwind generates a rule for. See
// `arbitrary-catalog.ts`.
//
// Reported apart from the main figure rather than folded into it, because
// the two denominators mean different things: one is "every utility that
// exists", the other is "a cross-section of a syntax with no bounds". Adding
// them would produce a number that answers neither question.
const arbitrary = await buildArbitraryCatalog()
const arbitraryResults = arbitrary.candidates.map((candidate) =>
  compareCandidate(candidate, arbitrary.oracle.rules.get(candidate), arbitrary.oracle.registerDefaults),
)
const arbitraryCount = (verdict: Verdict) =>
  arbitraryResults.filter((result) => result.verdict === verdict).length
const arbitraryMismatches = arbitraryResults.filter((r) => r.verdict === 'MISMATCH')
console.log(
  `\n== Arbitrary values (${arbitrary.candidates.length} candidates Tailwind generates) ==\n` +
    `Match:       ${arbitraryCount('MATCH')}\n` +
    `Mismatch:    ${arbitraryMismatches.length}\n` +
    `Unsupported: ${arbitraryCount('UNSUPPORTED')}\n` +
    `Skipped:     ${arbitraryCount('SKIPPED')}   (one side unresolvable; no claim made)`,
)
for (const mismatch of arbitraryMismatches) {
  console.log(`  ${mismatch.candidate.padEnd(34)} ${mismatch.detail ?? ''}`)
}

// == Compositions =========================================================
//
// The third denominator, and the one that measures what the other two
// can't. Every section above compares one utility at a time, which is
// exactly wrong for the families Tailwind builds out of several: the
// gradient constructors and their stops, the ring and shadow layers, the
// filter chain, the transform axes. All of those come back
// COMPOSITION_ONLY one at a time -- a true verdict that leaves 971
// gradient entries alone unmeasured.
//
// Hand-written rather than derived, unlike the other two, because there
// is nothing to derive it from: Tailwind enumerates its utilities and
// not the interactions between them. See `compositions.ts`.
const compositions = await buildCompositionCatalog()
const compositionVars = new Map([...loadThemeVars(), ...compositions.registerDefaults])
const compositionResults = compositions.candidates.map((candidate) =>
  compareCandidate(candidate, compositions.expected.get(candidate), compositionVars),
)
const compositionCount = (verdict: Verdict) =>
  compositionResults.filter((result) => result.verdict === verdict).length
console.log(
  `\n\n== Compositions (${compositions.candidates.length} combinations) ==\n` +
    'Several utilities reaching one declaration, which a one-utility comparison\n' +
    'reports as composition-only and never checks.\n\n' +
    `Match:       ${compositionCount('MATCH')}\n` +
    `Mismatch:    ${compositionCount('MISMATCH')}\n` +
    `Unsupported: ${compositionCount('UNSUPPORTED')}\n` +
    `Skipped:     ${compositionCount('SKIPPED')}   (one side unresolvable; no claim made)\n` +
    `Inert:       ${compositionCount('COMPOSITION_ONLY')}   (still paints nothing even combined)`,
)
for (const result of compositionResults) {
  if (result.verdict === 'MATCH') continue
  console.log(`  ${result.verdict.padEnd(17)} ${result.candidate}\n    ${result.detail ?? ''}`)
}

// == Variants =============================================================
//
// The fourth denominator, and the only one that checks *where* a rule
// applies. Everything above matches declaration text, so a rule that lost
// its `@media` wrapper reads as identical to one that kept it -- and
// Tailwind's `getClassList()` enumerates utilities without the variants in
// front of them, so no derived list contains `md:hover:flex` either.
// Between the two, stacked variants compiled to nothing at all and nothing
// here could have said so. See `variants.ts`.
const variants = await buildVariantCatalog()
const variantResults = variants.cases.map((entry) => compareVariant(entry, variants.vars))
const variantCount = (verdict: string) =>
  variantResults.filter((result) => result.verdict === verdict).length
console.log(
  `\n\n== Variants (${variants.cases.length} single and stacked) ==\n` +
    'Declarations, the selector matched, and the at-rules around it.\n\n' +
    `Match:       ${variantCount('MATCH')}\n` +
    `Mismatch:    ${variantCount('MISMATCH')}\n` +
    `Unsupported: ${variantCount('UNSUPPORTED')}   (Dowel emits no rule)`,
)
for (const result of variantResults) {
  if (result.verdict === 'MATCH') continue
  console.log(`  ${result.verdict.padEnd(12)} ${result.candidate}\n    ${result.detail ?? ''}`)
}
