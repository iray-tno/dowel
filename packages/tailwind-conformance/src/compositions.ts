// A denominator for the families that only paint when several utilities
// are written together.
//
// The report everywhere else compares one utility at a time, which is the
// right shape for most of Tailwind and says nothing at all about the rest:
// `from-red-500` sets a register, `bg-linear-to-r` reads one, and neither
// alone produces a declaration a browser keeps. The verdict for both is
// COMPOSITION_ONLY -- accurate, and it means 971 gradient entries have
// never been measured by anything.
//
// So this section asks the same question of *combinations*. The oracle
// already compiles each class; the expected CSS for a combination is its
// classes' declarations concatenated in Tailwind's own emission order,
// which is the order the cascade resolves them in for equal specificity.
// Dowel's side needs no special handling at all -- it has always taken a
// whole class attribute.
//
// The list is hand-written, unlike every other denominator here, and that
// is a real weakness: a combination nobody thought of is a combination
// nobody measures. It is bounded by what can be derived -- there is no
// enumeration of "utility pairs that interact" to ask Tailwind for -- so
// the entries are chosen to cover each composing family's shapes rather
// than its values.

import { buildOracle } from './oracle.ts'

/**
 * Combinations, one per line, each a whole class attribute.
 *
 * Grouped by the mechanism being exercised rather than by utility name:
 * what is under test is that several utilities reach one declaration,
 * so the interesting axis is how they combine.
 */
export const COMPOSITIONS: string[] = [
  // Gradients: constructor plus stops. The constructor decides the
  // function and the prelude; the stops decide everything inside it.
  'bg-linear-to-r from-red-500 to-blue-500',
  'bg-linear-to-b from-red-500 to-blue-500',
  'bg-linear-to-tl from-red-500 to-blue-500',
  'bg-linear-45 from-red-500 to-blue-500',
  'bg-linear-180 from-red-500 to-blue-500',
  'bg-radial from-red-500 to-blue-500',
  'bg-conic from-red-500 to-blue-500',
  'bg-conic-90 from-red-500 to-blue-500',
  'bg-gradient-to-r from-red-500 to-blue-500',
  // A middle stop, which changes the shape of the list rather than a
  // value in it.
  'bg-linear-to-r from-red-500 via-green-500 to-blue-500',
  // Positions, together with and apart from colours.
  'bg-linear-to-r from-red-500 from-20% to-blue-500 to-80%',
  'bg-linear-to-r from-red-500 via-green-500 via-30% to-blue-500',
  // A via *position* with no via colour: Tailwind keeps those in separate
  // registers, and the stop stays out of the list.
  'bg-linear-to-r from-red-500 via-30% to-blue-500',
  // Only one end written. The other is `#0000`, so this is a ramp to
  // transparent rather than a half-finished gradient.
  'bg-linear-to-r from-red-500',
  'bg-linear-to-r to-blue-500',
  // The interpolation modifier, which is part of the constructor.
  'bg-linear-to-r/srgb from-red-500 to-blue-500',
  'bg-linear-to-r/oklch from-red-500 to-blue-500',
  'bg-linear-to-r/longer from-red-500 to-blue-500',
  // Arbitrary values on both halves.
  'bg-linear-[25deg] from-[#123456] to-[#654321]',
  'bg-radial-[at_top_left] from-red-500 to-blue-500',
  // Later utility wins, which only means anything if both reached the
  // same declaration.
  'bg-linear-to-r from-red-500 from-blue-500 to-green-500',

  // Rings and shadows: four layers sharing one `box-shadow`.
  'shadow-lg ring-2',
  'shadow-lg ring-2 ring-blue-500',
  'ring-2 ring-offset-2 ring-offset-white',
  'inset-shadow-sm shadow-lg',
  'shadow-lg shadow-blue-500',
  'inset-ring-2 inset-ring-red-500 ring-2 ring-blue-500',
  'shadow-none ring-2',

  // Filters: nine registers spliced into one chain, in a fixed order that
  // is not the order they were written.
  'grayscale invert',
  'invert grayscale',
  'blur-sm brightness-125 saturate-150',
  'backdrop-blur-sm backdrop-grayscale',
  'blur-sm backdrop-blur-lg',
  'drop-shadow-lg blur-sm',

  // Transforms: axes composing into `translate`, `scale` and `transform`.
  'translate-x-4 translate-y-8',
  'translate-x-4 translate-y-8 translate-z-2',
  'scale-x-50 scale-y-75',
  'scale-50 scale-x-75',
  'rotate-x-45 skew-y-6',
  'rotate-45 scale-110 translate-x-2',

  // Masks: slots and stops, the family that already resolved standalone
  // because its registers carry `@property` defaults.
  'mask-t-from-50% mask-t-to-90%',
  'mask-x-from-20% mask-y-to-80%',
  'mask-linear-45 mask-linear-from-30%',
  'mask-radial-closest-side mask-radial-from-40%',

  // The two-axis one-declaration families.
  'border-spacing-x-2 border-spacing-y-4',
  'scrollbar-thumb-red-500 scrollbar-track-blue-500',

  // Last-wins across a shorthand and its longhand, which is the ordering
  // question `dedupe_last_wins` exists to answer.
  'p-4 pt-8',
  'px-4 pl-8',
  'border-2 border-t-4',
  'rounded-lg rounded-tl-none',
]

export interface CompositionCatalog {
  candidates: string[]
  /** The CSS Tailwind produces for each whole combination. */
  expected: Map<string, string>
  registerDefaults: Map<string, string>
}

export async function buildCompositionCatalog(): Promise<CompositionCatalog> {
  const classes = [...new Set(COMPOSITIONS.flatMap((c) => c.split(/\s+/)))]
  const oracle = await buildOracle(classes)
  // Tailwind's emission order, which is what decides the winner between
  // two declarations of equal specificity. The order the classes appear
  // in the attribute is *not* it -- `p-4 pt-8` and `pt-8 p-4` render the
  // same, because the stylesheet is what orders them.
  const order = new Map([...oracle.rules.keys()].map((name, index) => [name, index]))

  const expected = new Map<string, string>()
  for (const composition of COMPOSITIONS) {
    const declarations = composition
      .split(/\s+/)
      .filter((name) => oracle.rules.has(name))
      .sort((a, b) => (order.get(a) ?? 0) - (order.get(b) ?? 0))
      .map((name) => oracle.rules.get(name)!)
      .join('')
    expected.set(composition, declarations)
  }
  return { candidates: COMPOSITIONS, expected, registerDefaults: oracle.registerDefaults }
}
