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
// So the list travels into the compiler, which resolves each tag against
// the module its binding came from. A tag from a module the project does
// not trust is carried verbatim -- the same treatment any component Hozo
// does not model already gets -- and the tree around it still compiles.
//
// Per tag rather than per file, and that is the whole of `@expo/ui`
// support. A real Expo app writes `<View className="p-4">` from
// `react-native` and `<Button label="Save">` from `@expo/ui` in one tree,
// and `@expo/ui` exports `Text`, `Button`, `List`, `ListItem`,
// `ScrollView` and `TextInput` -- every one a native platform component
// sharing nothing with the Hozo primitive but its spelling. Refusing the
// file would leave the half Hozo understands uncompiled; accepting it
// would replace a SwiftUI button with a `<div>`.
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

/**
 * Primitive-named bindings this file imports from a module not on the list.
 *
 * The same rule the compiler applies per tag, available to a backend that
 * has to reason about what came *out* of it. The Native transform refuses
 * a `<Button>` that survived lowering, because Hozo's Button is a semantic
 * primitive and React Native's takes a `title` and renders no children --
 * neither works carried. But `@expo/ui` exports a `Button` too, and a
 * carried one of those is the correct outcome rather than a failure.
 */
export function foreignPrimitives(source: string, allowed: readonly string[]): Set<string> {
  return new Set(
    primitiveImports(source)
      .filter((entry) => !allowed.includes(entry.module))
      .map((entry) => entry.local),
  )
}
