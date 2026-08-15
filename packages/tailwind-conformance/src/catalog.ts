// The uncurated denominator: every utility Tailwind itself can generate.
//
// `candidates.ts` holds a hand-picked slice of what real app code uses,
// which is useful for seeing whether the common path works -- but a score
// out of a list we chose ourselves is close to grading our own homework.
// This asks Tailwind to enumerate its own surface instead, through the same
// entry point the official IntelliSense extension uses.
//
// It is a much harsher denominator, and worth reading with its shape in
// mind rather than as one number: of the ~23k entries, a single family
// (`mask-*`) is over a quarter, and colour utilities multiply one code path
// by 22 families x 11 shades. Covering `bg-blue-500` and `bg-blue-600` is
// the same work. So the per-namespace table is the actionable view; the
// bare percentage mostly measures how combinatorial Tailwind's value
// expansion is.

import { readFileSync } from 'node:fs'
import path from 'node:path'
import { __unstable__loadDesignSystem } from 'tailwindcss'
import { tailwindPackageDir } from './theme.ts'

/** Every class name Tailwind's design system can produce, unfiltered. */
export async function loadFullCatalog(): Promise<string[]> {
  const dir = tailwindPackageDir()
  const design = await __unstable__loadDesignSystem('@import "tailwindcss";', {
    base: dir,
    loadStylesheet: async (id: string) => {
      const file = id === 'tailwindcss' ? path.join(dir, 'index.css') : id
      return { path: file, base: path.dirname(file), content: readFileSync(file, 'utf8') }
    },
  })
  return design.getClassList().map(([name]: [string, unknown]) => name)
}

/**
 * The family a utility belongs to, for grouping the report.
 *
 * The leading segment, with the negative marker dropped so `-mt-4` counts
 * as `mt` rather than as its own thing. Crude, and deliberately so: it's
 * derived from the names Tailwind emits rather than from a mapping we'd
 * have to maintain, which is the whole point of this denominator.
 */
export function namespaceOf(candidate: string): string {
  return candidate.replace(/^-/, '').split('-')[0]
}
