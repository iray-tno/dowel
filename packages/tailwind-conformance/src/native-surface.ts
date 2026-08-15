// React Native's own enumeration of what a style can contain.
//
// The Native half of this report rests on Dowel's `unsupported_on_native()`
// -- roughly ten thousand refusals, every one of them Dowel's own say-so.
// That is the same "grading our own homework" problem the full Tailwind
// catalogue solved for the Web half, and it wants the same solution: ask
// the other tool to enumerate itself.
//
// So this reads the style surface out of `react-native`'s shipped `.d.ts`.
// It is a regex over type declarations rather than a TypeScript parse,
// which is coarse -- but these interfaces are one property per line and
// plainly formatted, and it is checked by a test that asserts a handful of
// known keys and known-absent keys, so drift shows up as a failure rather
// than as a quietly shrinking surface.

import { existsSync, readFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)

/** Directory of the installed `react-native` package. */
export function reactNativeDir(): string {
  return path.dirname(require.resolve('react-native/package.json'))
}

/** The React Native version the surface was read from. */
export function reactNativeVersion(): string {
  const pkg = JSON.parse(readFileSync(path.join(reactNativeDir(), 'package.json'), 'utf8'))
  return pkg.version as string
}

/**
 * What React Native accepts for one style key.
 *
 * `values` is set only when the declared type is a closed union of string
 * literals -- `display` is `'none' | 'flex' | 'contents'`, so a refusal of
 * `display: grid` is defensible even though `display` itself exists. When
 * the type admits numbers, colours or dimensions there is no useful
 * value-level check, and `values` stays undefined.
 */
export interface StyleKey {
  name: string
  values?: Set<string>
}

/// Only the interfaces that describe a style. The file also declares the
/// shapes of things that appear *inside* a style value -- `BoxShadowValue`
/// has `blurRadius`, `offsetX`, `spreadDistance`; the transform interfaces
/// have `scaleX`, `translateY` -- and those are not keys you can write at
/// the top level of a StyleSheet entry. Including them would manufacture
/// suspect refusals for properties React Native does not actually have.
const STYLE_INTERFACES = ['FlexStyle', 'ShadowStyleIOS', 'ViewStyle', 'TextStyleIOS', 'TextStyleAndroid', 'TextStyle', 'ImageStyle']

function styleTypesSource(): string {
  return readFileSync(
    path.join(reactNativeDir(), 'Libraries/StyleSheet/StyleSheetTypes.d.ts'),
    'utf8',
  )
}

/**
 * Every style key React Native's types declare, across every style
 * interface.
 *
 * Deliberately the union of all of them: Dowel refuses a property because
 * React Native has no such style *at all*, not because it is unavailable on
 * one primitive. A per-primitive restriction is a different (and narrower)
 * claim, and the report already reports that separately as `restrictedTo`.
 */
export function reactNativeStyleKeys(): Map<string, StyleKey> {
  const source = styleTypesSource()
  const aliases = typeAliases(source)
  const keys = new Map<string, StyleKey>()

  for (const body of styleInterfaceBodies(source)) {
    // `  borderTopWidth?: number | undefined;` -- an identifier at one
    // indent level, then everything up to the terminating semicolon, which
    // may be several lines down for a wide union.
    for (const match of body.matchAll(/^ {2}(?:readonly )?([a-zA-Z][a-zA-Z0-9]*)\??:([^;]*);/gm)) {
      const [, name, type] = match
      const values = literalUnion(type, aliases)
      // A key seen in two interfaces keeps the looser constraint: `overflow`
      // is narrower on ImageStyle than on ViewStyle, and the question here
      // is whether React Native can express it anywhere.
      const existing = keys.get(name)
      if (existing && (existing.values === undefined || values === undefined)) {
        keys.set(name, { name })
        continue
      }
      if (existing?.values && values) {
        keys.set(name, { name, values: new Set([...existing.values, ...values]) })
        continue
      }
      keys.set(name, values ? { name, values } : { name })
    }
  }
  return keys
}

/** The body text of each interface in `STYLE_INTERFACES`. */
function styleInterfaceBodies(source: string): string[] {
  const bodies: string[] = []
  for (const name of STYLE_INTERFACES) {
    const start = source.search(new RegExp(`^(?:export )?interface ${name}\\b[^{]*\\{$`, 'm'))
    if (start === -1) continue
    const open = source.indexOf('{', start)
    // These interfaces contain no nested braces at the declaration level,
    // so the first `^}` closes them.
    const end = source.indexOf('\n}', open)
    bodies.push(source.slice(open + 1, end))
  }
  return bodies
}

/** `type X = ...;` declarations, so a property typed by name can resolve. */
function typeAliases(source: string): Map<string, string> {
  const aliases = new Map<string, string>()
  for (const match of source.matchAll(/^(?:export )?type ([A-Za-z][A-Za-z0-9]*)\s*=([^;]*);/gm)) {
    aliases.set(match[1], match[2])
  }
  return aliases
}

/**
 * The string literals a type admits, or undefined if it admits anything
 * else.
 *
 * `undefined` and `null` are dropped: every key is optional, and neither
 * says anything about which CSS values are reachable.
 */
function literalUnion(type: string, aliases: Map<string, string>, depth = 0): Set<string> | undefined {
  if (depth > 4) return undefined
  const values = new Set<string>()
  for (const part of type.split('|')) {
    const term = part.trim()
    if (term === '' || term === 'undefined' || term === 'null') continue
    const literal = /^'([^']*)'$/.exec(term)
    if (literal) {
      values.add(literal[1])
      continue
    }
    const alias = aliases.get(term)
    if (alias) {
      const nested = literalUnion(alias, aliases, depth + 1)
      if (nested === undefined) return undefined
      for (const value of nested) values.add(value)
      continue
    }
    // `number`, `ColorValue`, `DimensionValue`, a template literal --
    // anything that isn't a closed set of names.
    return undefined
  }
  return values.size > 0 ? values : undefined
}

// ---------------------------------------------------------------------------
// Second opinion: react-native-css
// ---------------------------------------------------------------------------

/// A peer project solving the same problem, used as a cross-check rather
/// than an authority: where it converts a CSS property that Dowel refuses,
/// that refusal is worth re-reading even if React Native's types agree with
/// us. Read from a clone under `temp/` (gitignored), so this is optional --
/// absent, the audit just runs without the second column.
///
/// Cloned at f70c402 (2026-07-13).
const REACT_NATIVE_CSS_CLONE = 'temp/react-native-css/src/compiler/declarations.ts'

function repoRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')
}

/**
 * The CSS properties react-native-css has a parser for -- its own answer to
 * "what can be lowered to a React Native style", keyed the way Dowel's
 * refusals are (by CSS property, not by RN style key).
 */
export function reactNativeCssProperties(): Set<string> | undefined {
  const file = path.join(repoRoot(), REACT_NATIVE_CSS_CLONE)
  if (!existsSync(file)) return undefined
  const source = readFileSync(file, 'utf8')

  // The table is `const parsers: {...} = { "align-content": parseAlignContent, ... }`.
  const start = source.indexOf('const parsers:')
  if (start === -1) return undefined
  const open = source.indexOf('= {', start)
  const end = source.indexOf('\n};', open)
  const body = source.slice(open, end)

  const properties = new Set<string>()
  for (const match of body.matchAll(/^\s+"([a-z-]+)":/gm)) {
    properties.add(match[1])
  }
  return properties.size > 0 ? properties : undefined
}

/** `border-top-width` -> `borderTopWidth`. */
export function camelCase(property: string): string {
  return property.replace(/-([a-z])/g, (_, ch: string) => ch.toUpperCase())
}
