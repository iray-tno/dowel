// The React Native glue for `./ambient.ts`. Deliberately thin -- all the
// logic worth testing lives there, free of `react`/`react-native` imports
// so it can be tested without a device or a native module registry. This
// file is the part that can only be verified by running an app, same
// division as `@dowel/metro-transformer`'s `index.ts`.
//
// Generated components call these; nothing here is meant to be imported by
// hand.

import { useEffect, useMemo, useRef, useSyncExternalStore } from 'react'
import { Animated, Appearance, Dimensions, Easing } from 'react-native'

import {
  bucketFor,
  createStore,
  isAtLeast,
  sameViewport,
  type BreakpointName,
  type Viewport,
} from './ambient.ts'

/// `Dimensions` hands back more than the two numbers wanted here (`scale`,
/// `fontScale`), and a fresh object every event. Narrowing it is what makes
/// the store's change check able to see that nothing moved.
function viewportOf({ width, height }: { width: number; height: number }): Viewport {
  return { width, height }
}

// One subscription per app, not per component. See ./ambient.ts.

const darkStore = createStore(Appearance.getColorScheme() === 'dark')
Appearance.addChangeListener(({ colorScheme }) => {
  darkStore.set(colorScheme === 'dark')
})

const breakpointStore = createStore(bucketFor(Dimensions.get('window').width))
// A second store over the same event, holding the size itself rather than
// the bucket. Kept separate on purpose: a component using only `md:`
// must not re-render on every resize that doesn't cross a breakpoint, and
// it wouldn't if these shared one snapshot.
const viewportStore = createStore(viewportOf(Dimensions.get('window')), sameViewport)
Dimensions.addEventListener('change', ({ window }) => {
  breakpointStore.set(bucketFor(window.width))
  viewportStore.set(viewportOf(window))
})

/** Whether the OS is in dark mode. Drives `dark:` utilities. */
export function useDowelDark(): boolean {
  return useSyncExternalStore(darkStore.subscribe, darkStore.get, darkStore.get)
}

/**
 * Whether the viewport is at least as wide as `name`'s breakpoint. Drives
 * `sm:`/`md:`/`lg:`/`xl:`/`2xl:` utilities.
 *
 * Takes the name rather than returning the current bucket so the call
 * reads as the condition it was compiled from: `md:` becomes
 * `useDowelBreakpoint('md')`.
 */
export function useDowelBreakpoint(name: BreakpointName): boolean {
  const bucket = useSyncExternalStore(
    breakpointStore.subscribe,
    breakpointStore.get,
    breakpointStore.get,
  )
  return isAtLeast(bucket, name)
}

/**
 * The current window size. Drives the viewport-relative sizes -- `h-screen`
 * compiles to `{ height: useDowelViewport().height }`.
 *
 * The window, not the screen: the window excludes system UI the app can't
 * draw under, which is the closer analogue of the Web viewport. It does not
 * subtract a notch or a home indicator, so a full-bleed layout still wants
 * a safe-area inset on top of this -- exactly as it does on Web.
 *
 * Unlike the two above, this re-renders on every window change rather than
 * only when a breakpoint is crossed. That is unavoidable for a size that
 * has to track the window exactly, and it's why this is a separate store
 * rather than the breakpoints being rebuilt on top of it.
 */
export function useDowelViewport(): Viewport {
  return useSyncExternalStore(viewportStore.subscribe, viewportStore.get, viewportStore.get)
}

/** Tailwind's `animate-spin`: one clockwise turn per second, forever. */
export function useDowelSpin() {
  const progress = useRef(new Animated.Value(0)).current

  useEffect(() => {
    const animation = Animated.loop(
      Animated.timing(progress, {
        toValue: 1,
        duration: 1000,
        easing: Easing.linear,
        useNativeDriver: true,
      }),
    )
    animation.start()
    return () => animation.stop()
  }, [progress])

  return useMemo(
    () => ({
      transform: [
        {
          rotate: progress.interpolate({
            inputRange: [0, 1],
            outputRange: ['0deg', '360deg'],
          }),
        },
      ],
    }),
    [progress],
  )
}
