// Ad-hoc: what Tailwind emits for a handful of classes, resolved the way
// the report resolves it. `node src/probe.ts <class> [<class> ...]`.
//
// Each argument is one candidate; quote a space-separated group to ask
// what a *combination* produces. Not part of the report -- this is the
// tool for the "what does the reference engine actually do here?" step
// that every fix in this package starts with.
import { buildOracle } from './oracle.ts'
import { normalize } from './normalize.ts'
import { loadThemeVars } from './theme.ts'

const groups = process.argv.slice(2).map((arg) => arg.split(/\s+/))
const oracle = await buildOracle(groups.flat())
const vars = new Map([...loadThemeVars(), ...oracle.registerDefaults])

for (const group of groups) {
  const raw = group.map((name) => oracle.rules.get(name) ?? '').join('')
  console.log(`\n${group.join(' ')}`)
  console.log(`  raw:      ${raw.replace(/\s+/g, ' ').trim() || '(no rule)'}`)
  const resolved = normalize(raw, vars)
  console.log(
    `  resolved: ${[...resolved.declarations].map(([p, v]) => `${p}: ${v}`).join('; ') || '(none)'}`,
  )
  if (resolved.unresolved.length > 0) console.log(`  unresolved: ${resolved.unresolved.join('; ')}`)
}
