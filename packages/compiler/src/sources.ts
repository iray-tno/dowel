// Which modules a project's primitives may come from.
//
// The compiler matches on the JSX tag name and never asks where the name
// was imported from. That is deliberate and it is what makes proposal §2.1
// true -- an existing React Native file compiles as written, with no
// migration to a Hozo-specific API:
//
//     import { View, Text } from 'react-native'
//
// It is also, unguarded, a way to be badly wrong. A `<View>` from some
// other component library has its own props, its own layout, and its own
// idea of what it renders, and lowering it to a `<div>` because the tag
// happens to be spelled `View` would silently replace someone's component
// with something else.
//
// So the compiler stays tag-based and this decides. A file is lowered when
// every primitive-named binding in it comes from a module the project
// trusts, and left alone when one doesn't.
//
// Left alone *quietly* when it trusts none of them: a project whose own
// components happen to be named `View` is not doing anything wrong, and a
// warning on each of its files would be noise about a decision Hozo was
// never asked to make. The diagnostic is for the mixed file -- one that
// imports from a module Hozo does handle and one it doesn't -- where the
// author has every reason to expect lowering and the reason it did not
// happen is a single import they can see.
//
// The integrations all defaulted to `@hozo/core` only, by way of a
// `code.includes('@hozo/core')` substring test. That skipped every Expo and
// React Native project on the grounds that they had not been rewritten.

import { primitiveImports } from './index.ts'

/**
 * Modules whose primitives Hozo lowers unless a project says otherwise.
 *
 * `react-native` is here because the compiler already handles it: the same
 * source compiles to the same output whichever of the two it was imported
 * from. Nothing had to change in the compiler to support Expo -- only the
 * gate in front of it.
 */
export const DEFAULT_PRIMITIVE_SOURCES = ['@hozo/core', 'react-native'] as const

export interface SourceDecision {
  /** Whether this file's primitives may be lowered. */
  compilable: boolean
  /**
   * Primitive-named bindings from a module not on the list.
   *
   * Non-empty means `compilable` is false: one unrecognised `View` makes
   * the whole file unsafe to lower, because the compiler cannot tell two
   * `<View>` tags apart.
   */
  foreign: { local: string; module: string }[]
}

/**
 * Decides whether a file may be lowered, and says what stopped it.
 *
 * A file importing no primitives at all is `compilable` with nothing to
 * lower -- the caller finds that out from the compiler, which is the only
 * thing that can tell a file with no JSX from one whose JSX is all
 * unrecognised.
 */
export function decideSources(source: string, allowed: readonly string[]): SourceDecision {
  const foreign = primitiveImports(source).filter((entry) => !allowed.includes(entry.module))
  return { compilable: foreign.length === 0, foreign }
}

/**
 * The message for a file Hozo declined to lower.
 *
 * Names the modules rather than the tags: the tag is `View` in both the
 * case that works and the case that doesn't, so it carries no information
 * about which this is.
 */
export function foreignSourceMessage(
  file: string,
  foreign: { local: string; module: string }[],
  allowed: readonly string[],
): string {
  const names = [...new Set(foreign.map((entry) => `\`${entry.local}\` from \`${entry.module}\``))]
  return (
    `${file} imports ${names.join(', ')}, and Hozo only lowers primitives from ` +
    `${allowed.map((name) => `\`${name}\``).join(', ')}. The file is left as written, which is ` +
    `correct if that is a different component with the same name -- and if it is a re-export of ` +
    `one Hozo does understand, add its module to the \`sources\` option.`
  )
}
