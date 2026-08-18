// Ad-hoc: the arbitrary candidates Hozo gets wrong, emits nothing for, or
// the harness can't judge. `node src/gaps.ts [SKIPPED]` -- not part of the
// report.
import { buildArbitraryCatalog } from './arbitrary-catalog.ts'
import { compareCandidate } from './compare.ts'

const want = new Set(process.argv.slice(2).length ? process.argv.slice(2) : ['UNSUPPORTED', 'MISMATCH'])
const arbitrary = await buildArbitraryCatalog()
const groups = new Map<string, { candidate: string; verdict: string; detail: string }[]>()
const counts = new Map<string, number>()
for (const candidate of arbitrary.candidates) {
  const result = compareCandidate(
    candidate,
    arbitrary.oracle.rules.get(candidate),
    arbitrary.oracle.registerDefaults,
  )
  counts.set(result.verdict, (counts.get(result.verdict) ?? 0) + 1)
  if (!want.has(result.verdict)) continue
  const prefix = candidate.replace(/-?[\[(].*$/s, '') || candidate
  groups.set(prefix, [
    ...(groups.get(prefix) ?? []),
    {
      candidate,
      verdict: result.verdict,
      detail:
        result.verdict === 'UNSUPPORTED'
          ? (arbitrary.oracle.rules.get(candidate) ?? '').replace(/\s+/g, ' ').trim()
          : (result.detail ?? ''),
    },
  ])
}

for (const [prefix, list] of [...groups].sort((a, b) => b[1].length - a[1].length)) {
  console.log(`\n${prefix}  (${list.length})`)
  for (const entry of list) {
    console.log(`    ${entry.verdict[0]} ${entry.candidate.padEnd(30)} ${entry.detail.slice(0, 240)}`)
  }
}
console.log(`\n${arbitrary.candidates.length} candidates: ${[...counts].map(([v, n]) => `${v} ${n}`).join(', ')}`)
console.log(`${groups.size} prefixes listed`)
