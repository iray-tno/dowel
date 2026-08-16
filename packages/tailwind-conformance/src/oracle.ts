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
    // Read the class names *out of* the selector and look each one up,
    // rather than testing every candidate against every selector. The
    // latter is fine for a hand-picked hundred and quadratic against the
    // full ~23k catalogue, where it is tens of millions of string scans.
    //
    // A selector may carry a pseudo-class or be a descendant form
    // (`:where(.space-x-2 > :not(:last-child))`), so every class in it is
    // considered, not just a whole-selector match.
    for (const escaped of classNamesIn(selector)) {
      const candidate = byName.get(escaped)
      if (candidate === undefined) continue
      rules.set(candidate, (rules.get(candidate) ?? '') + declarations)
    }
  }
  return { rules, registerDefaults: extractRegisterDefaults(css) }
}

/**
 * The escaped class names a selector targets.
 *
 * Backslash escapes are part of the name (`.hover\:bg-blue-500:hover` is
 * the class `hover\:bg-blue-500` followed by a pseudo-class), so an escaped
 * character is consumed together with its backslash -- otherwise the `:` in
 * `\:` would look like the start of a pseudo-class and cut the name short.
 */
function classNamesIn(selector: string): string[] {
  const names: string[] = []
  for (let i = 0; i < selector.length; i += 1) {
    if (selector[i] !== '.') continue
    let name = ''
    let j = i + 1
    while (j < selector.length) {
      const ch = selector[j]
      if (ch === '\\' && j + 1 < selector.length) {
        name += ch + selector[j + 1]
        j += 2
        continue
      }
      if (!/[\w-]/.test(ch)) break
      name += ch
      j += 1
    }
    if (name) names.push(name)
    i = j - 1
  }
  return names
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
  // Everything CSS doesn't allow bare in an identifier, which is the rule
  // rather than a list -- the list kept being wrong. It started as `[:/.]`,
  // which was complete for the named-utility catalogue and silently wrong
  // for arbitrary values: every one of them looked up to nothing, so the
  // oracle reported "Tailwind emits no rule for this" when what had
  // happened was that this function couldn't spell the selector. Widening
  // it to brackets fixed most of them and still hid `*`, `+` and quotes,
  // which read as three more Tailwind limitations that weren't real.
  //
  // Non-ASCII is left alone: CSS allows it in an identifier unescaped, and
  // Tailwind writes it through.
  return candidate.replace(/[^\w-]/g, (ch) => (ch.charCodeAt(0) > 127 ? ch : `\\${ch}`))
}
