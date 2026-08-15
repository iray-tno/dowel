// Which children `space-*` and `divide-*` reach.
//
// On Web these compile to `:where(.cls > :not(:last-child))` and the browser
// answers the question at match time. React Native has no selector engine,
// so the answer has to be computed -- and computed the same way, or the two
// backends lay out differently from the same source.
//
// The subtlety worth having a testable function for is what counts as a
// child. `:not(:last-child)` is an *element* selector: a text node at the
// end of the parent is not `:last-child`, so the last element still gets no
// margin. `React.Children.toArray` keeps strings in the array, so taking
// "everything but the final entry" would disagree with Web exactly when a
// component ends with a stray line of text -- which is easy to write and
// hard to notice.
//
// Kept free of `react` imports so it can be tested without one; the glue
// lives in `./spacing.native.tsx`.

/**
 * The indices of `children` that should receive the spacing style: every
 * element except the last one.
 *
 * `isElement` is passed in rather than imported so this stays testable --
 * it's `React.isValidElement` at the call site.
 */
export function spacingTargets(
  children: readonly unknown[],
  isElement: (child: unknown) => boolean,
): number[] {
  const elements: number[] = []
  for (const [index, child] of children.entries()) {
    if (isElement(child)) elements.push(index)
  }
  // Nothing to space if there's one element or none: on Web the sole
  // element *is* `:last-child` and matches nothing.
  return elements.slice(0, -1)
}
