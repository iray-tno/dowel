// Wraps the real Tailwind engine as a correctness oracle: given a list of
// candidate class names it returns exactly the CSS each one should produce.

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { compile } from 'tailwindcss'
import { extractRules } from './extract.ts'
import { tailwindPackageDir } from './theme.ts'

export type OracleRules = Map<string, string>

export interface Oracle {
  rules: OracleRules
  /**
   * Initial values of the `--tw-*` registers Tailwind declares via
   * `@property`. Its utilities reference these without a fallback (e.g.
   * `box-shadow: var(--tw-ring-shadow), var(--tw-shadow)`), so resolving
   * one needs them -- read from the compiled output rather than hardcoded
   * so they can't drift from the version under test.
   */
  registerDefaults: Map<string, string>
}

/** Compiles `candidates` and returns each one's declaration block. */
export async function buildOracle(candidates: string[]): Promise<Oracle> {
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
  return { rules, registerDefaults: extractRegisterDefaults(css) }
}

function extractRegisterDefaults(css: string): Map<string, string> {
  const defaults = new Map<string, string>()
  const re = /@property\s+(--[a-z0-9-]+)\s*\{([^}]*)\}/gi
  let match: RegExpExecArray | null
  while ((match = re.exec(css))) {
    const initial = /initial-value:\s*([^;]+);/i.exec(match[2])
    if (initial) defaults.set(match[1], initial[1].trim())
  }
  return defaults
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
