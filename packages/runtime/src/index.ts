// The only thing Dowel ships to a device, and it exists for exactly one
// reason: React Native has no CSS engine.
//
// On Web, a `className` the compiler couldn't read needs no runtime at all
// -- the class string reaches the DOM, and the browser matches it against
// the generated candidate stylesheet by itself. Native has nothing to hand
// the string to, so this closes that one gap and nothing else.
//
// Compare `react-native-css`, which implements a genuine CSS engine in JS:
// specificity sorting, media/container query evaluation, CSS variables, a
// reactive observable graph. It needs all of that because it accepts
// arbitrary CSS. This resolver handles only *single Tailwind utility
// classes*, which are all the same specificity -- so "later in the string
// wins" is the entire cascade, and React Native's own style-array merging
// already implements that. Hence a lookup, not an engine.

/** A React Native style object, as the compiler emits it. */
export type StyleObject = Record<string, unknown>

/**
 * Resolves whitespace-separated class names to React Native style objects.
 *
 * Returns an array, which is a valid `style` value on its own and also
 * nests inside one -- RN flattens arbitrarily nested style arrays, so
 * callers never have to spread it.
 */
export type ClassResolver = (value: unknown) => StyleObject[]

/**
 * Builds a resolver over one project's candidate map.
 *
 * Called by the generated candidate module rather than by application
 * code: `styles` and `unsupported` are build output, and binding them here
 * keeps the generated file to data plus one call.
 *
 * `unsupported` maps a class the compiler *recognized but can't express*
 * as a plain style object -- a conditional utility (`hover:`, `md:`), or
 * one that's Web-only -- to the reason. Those are reported when they're
 * actually used rather than at build time, because a class appearing in
 * the project's candidate scan doesn't mean any dynamic expression ever
 * produces it.
 */
export function createClassResolver(
  styles: Record<string, StyleObject>,
  unsupported: Record<string, string> = {},
): ClassResolver {
  // Class strings repeat constantly across renders and list items, and the
  // result is immutable, so each distinct string is resolved once.
  const cache = new Map<string, StyleObject[]>()
  // Warn once per class, not once per render -- a warning inside a list's
  // render path would otherwise repeat for every row, every frame.
  const warned = new Set<string>()

  return function dowelClasses(value: unknown): StyleObject[] {
    // Falsy is the normal case, not an error: `cond && 'p-4'` yields
    // `false` whenever the condition is off.
    if (typeof value !== 'string' || value === '') {
      return []
    }

    const cached = cache.get(value)
    if (cached) {
      return cached
    }

    const resolved: StyleObject[] = []
    for (const name of value.split(/\s+/)) {
      if (name === '') {
        continue
      }
      const style = styles[name]
      if (style) {
        resolved.push(style)
        continue
      }
      // An unknown class isn't necessarily a Dowel problem -- it may be an
      // app's own non-Tailwind class, or a testID-ish marker -- so only the
      // ones the compiler recognized and refused are reported.
      const reason = unsupported[name]
      if (reason && !warned.has(name)) {
        warned.add(name)
        // eslint-disable-next-line no-console
        console.warn(`[dowel] ${reason}`)
      }
    }

    cache.set(value, resolved)
    return resolved
  }
}
