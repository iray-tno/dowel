// Reads a project's design tokens out of its Tailwind setup.
//
// This is what `@hozo/tailwind` is for (proposal §6.2): Tailwind is the
// frontend, the Style IR is the internal representation, and this is the
// boundary between them. The utility-to-IR translation lives in Rust and
// stays there; what a JavaScript package can do that Rust can't is ask
// Tailwind itself what the project's theme is.
//
// Asking rather than parsing matters. A `@theme` block can import other
// files, extend the default palette, or redefine part of it, and the only
// thing that resolves all of that correctly is Tailwind. So the project's
// CSS goes in, Tailwind's own design system comes out, and this reads the
// custom properties off it.

import { readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'

import { converter, formatHex } from 'culori'
import { __unstable__loadDesignSystem } from 'tailwindcss'

export interface ThemeColor {
  token: string
  /** As Tailwind writes it, which is what the Web backend emits verbatim. */
  oklch: string
  /** React Native's style system has no `oklch()`, so it takes this. */
  hex: string
}

export interface Theme {
  colors: ThemeColor[]
  /**
   * One spacing step in pixels. Tailwind's `--spacing` is a length, and
   * every spacing utility is a multiple of it, so a project that changes
   * it changes every padding, margin and gap at once -- which is why
   * getting this wrong was silent: the output was an ordinary padding, at
   * the wrong size.
   */
  spacingPx?: number
}

const toRgb = converter('rgb')

/**
 * Loads the theme a stylesheet defines.
 *
 * `css` is the project's entry stylesheet -- the file with
 * `@import "tailwindcss"` and its `@theme` block. `base` is the directory
 * imports resolve against, which is the file's own directory in every
 * ordinary setup.
 */
export async function loadTheme(css: string, base: string): Promise<Theme> {
  const design = await __unstable__loadDesignSystem(css, {
    base,
    loadStylesheet: async (id: string, from: string) => {
      // `tailwindcss` itself resolves to the installed package; everything
      // else is a path relative to the importer.
      const file =
        id === 'tailwindcss'
          ? path.join(tailwindPackageDir(), 'index.css')
          : path.resolve(path.dirname(from), id)
      return { path: file, base: path.dirname(file), content: readFileSync(file, 'utf8') }
    },
  })

  const colors: ThemeColor[] = []
  for (const [name, value] of design.theme.entries()) {
    if (!name.startsWith('--color-')) continue
    const oklch = String(value.value).trim()
    const hex = toHex(oklch)
    // A colour that won't convert is left out rather than guessed at. The
    // backends already have a defined answer for a token they can't
    // resolve -- a CSS variable reference on Web, a marker on Native --
    // and that is better than a colour that is nearly right.
    if (hex === null) continue
    colors.push({ token: name.slice('--color-'.length), oklch, hex })
  }
  return { colors, spacingPx: readSpacing(design) }
}

/**
 * `oklch(...)` to `#rrggbb`, or `null` if it isn't a colour this can
 * convert.
 *
 * Through `culori` rather than by hand: the same library, and the same
 * conversion, that produced Hozo's built-in copy of the default palette,
 * so a project's tokens and the built-ins can't disagree about what a
 * given oklch means.
 */
export function toHex(value: string): string | null {
  try {
    const rgb = toRgb(value)
    return rgb ? formatHex(rgb) : null
  } catch {
    return null
  }
}

export function tailwindPackageDir(): string {
  const require = createRequire(import.meta.url)
  return path.dirname(require.resolve('tailwindcss/package.json'))
}

/// The root font size CSS resolves `rem` against, and the one Tailwind's
/// own defaults assume.
const ROOT_FONT_SIZE_PX = 16

/**
 * `--spacing` in pixels, or `undefined` if the project leaves it alone.
 *
 * Undefined rather than the default: an absent value means "whatever Hozo
 * already does", which keeps a project that never touched the scale on
 * exactly the path it was on.
 */
function readSpacing(design: {
  theme: { entries(): Iterable<[string, { value: unknown }]> }
}): number | undefined {
  for (const [name, entry] of design.theme.entries()) {
    if (name !== '--spacing') continue
    const value = String(entry.value).trim()
    const rem = /^(-?[\d.]+)rem$/.exec(value)
    if (rem) return parseFloat(rem[1]!) * ROOT_FONT_SIZE_PX
    const px = /^(-?[\d.]+)px$/.exec(value)
    if (px) return parseFloat(px[1]!)
    // Anything else -- a `calc()`, a custom property chain -- is left to
    // the default rather than guessed at. Guessing here would scale every
    // spacing utility in the project by a number nobody chose.
    return undefined
  }
  return undefined
}
