// Wraps the real Tailwind engine as a correctness oracle: given a list of
// candidate class names it returns exactly the CSS each one should produce.

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { compile } from 'tailwindcss'
import { extractRules } from './extract.ts'
import { tailwindPackageDir } from './theme.ts'

export type OracleRules = Map<string, string>

/** Compiles `candidates` and returns each one's declaration block. */
export async function buildOracle(candidates: string[]): Promise<OracleRules> {
  const dir = tailwindPackageDir()
  const compiler = await compile('@import "tailwindcss";', {
    base: dir,
    loadStylesheet: async (id: string) => {
      const file = id === 'tailwindcss' ? path.join(dir, 'index.css') : id
      return { path: file, base: path.dirname(file), content: readFileSync(file, 'utf8') }
    },
  })

  const css = compiler.build(candidates)
  const utilities = css.slice(css.indexOf('@layer utilities'))

  const rules: OracleRules = new Map()
  const byName = new Map(candidates.map((c) => [escapeClassName(c), c]))

  for (const { selector, declarations } of extractRules(utilities)) {
    // A selector may carry a pseudo-class or be a descendant form
    // (`:where(.space-x-2 > :not(:last-child))`), so match against the
    // escaped class name rather than requiring the whole selector to equal
    // it.
    for (const [escaped, candidate] of byName) {
      if (!selectorTargetsClass(selector, escaped)) continue
      rules.set(candidate, (rules.get(candidate) ?? '') + declarations)
    }
  }
  return rules
}

/** How Tailwind escapes a candidate when writing it as a CSS selector. */
function escapeClassName(candidate: string): string {
  return candidate.replace(/[:/.]/g, (ch) => `\\${ch}`)
}

function selectorTargetsClass(selector: string, escapedClass: string): boolean {
  const idx = selector.indexOf(`.${escapedClass}`)
  if (idx === -1) return false
  // Guard against `.p-4` also matching inside `.p-40`: the next character
  // must not continue the class name.
  const next = selector[idx + escapedClass.length + 1]
  return next === undefined || !/[\w-]/.test(next)
}
