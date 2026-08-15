// Ambient conditions: the ones whose value is the same for the whole app
// at any moment -- colour scheme and viewport width.
//
// These are what make `dark:` and `md:` workable on React Native without
// the reactive engine Dowel deliberately doesn't ship. They aren't
// per-element state, so a single module-level subscription serves every
// component; each one only needs React to re-render it when the value
// changes, which is what `useSyncExternalStore` is for.
//
// Why not RN's own `useColorScheme()` / `useWindowDimensions()`: those
// subscribe *per component*. A list of 200 rows using `md:` would open 200
// subscriptions and re-render all 200 on every dimension event -- and on
// Android those fire on keyboard show/hide, not just rotation. Here there
// is one subscription, and the snapshot is a coarse string, so React's
// `Object.is` bail-out means a resize that doesn't cross a breakpoint
// re-renders nothing at all.
//
// This module is deliberately free of `react` and `react-native` imports
// so it can be tested without either. `./hooks.native.ts` is the glue that
// connects it to both.

type Listener = () => void

export interface Store<T> {
  get: () => T
  /** Notifies subscribers only when the value actually changes. */
  set: (next: T) => void
  subscribe: (listener: Listener) => () => void
}

export function createStore<T>(initial: T): Store<T> {
  const listeners = new Set<Listener>()
  let snapshot = initial
  return {
    get: () => snapshot,
    set(next: T) {
      if (Object.is(next, snapshot)) {
        return
      }
      snapshot = next
      for (const listener of listeners) {
        listener()
      }
    },
    subscribe(listener: Listener) {
      listeners.add(listener)
      return () => {
        listeners.delete(listener)
      }
    },
  }
}

/// Tailwind's default `min-width` breakpoints, widest first. Kept in step
/// with `dowel_ir::Breakpoint`, whose names the compiler emits.
export const BREAKPOINTS = [
  ['2xl', 1536],
  ['xl', 1280],
  ['lg', 1024],
  ['md', 768],
  ['sm', 640],
] as const

export type BreakpointName = (typeof BREAKPOINTS)[number][0]

/**
 * The widest breakpoint `width` satisfies, or `''` below all of them.
 *
 * A single coarse string rather than the raw width, and that is the whole
 * point: it makes the store's change check meaningful. Resizing within one
 * bucket produces an identical snapshot, so nothing re-renders.
 */
export function bucketFor(width: number): BreakpointName | '' {
  for (const [name, min] of BREAKPOINTS) {
    if (width >= min) {
      return name
    }
  }
  return ''
}

/** Whether `bucket` is at least as wide as the `name` breakpoint. */
export function isAtLeast(bucket: BreakpointName | '', name: BreakpointName): boolean {
  if (bucket === '') {
    return false
  }
  // Ascending width is descending index in BREAKPOINTS.
  const indexOf = (want: string) => BREAKPOINTS.findIndex(([n]) => n === want)
  return indexOf(bucket) <= indexOf(name)
}
