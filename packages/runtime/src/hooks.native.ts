// The React Native glue for `./ambient.ts`. Deliberately thin -- all the
// logic worth testing lives there, free of `react`/`react-native` imports
// so it can be tested without a device or a native module registry. This
// file is the part that can only be verified by running an app, same
// division as `@dowel/metro-transformer`'s `index.ts`.
//
// Generated components call these; nothing here is meant to be imported by
// hand.

import { useSyncExternalStore } from 'react'
import { Appearance, Dimensions } from 'react-native'

import { bucketFor, createStore, isAtLeast, type BreakpointName } from './ambient.ts'

// One subscription per app, not per component. See ./ambient.ts.

const darkStore = createStore(Appearance.getColorScheme() === 'dark')
Appearance.addChangeListener(({ colorScheme }) => {
  darkStore.set(colorScheme === 'dark')
})

const breakpointStore = createStore(bucketFor(Dimensions.get('window').width))
Dimensions.addEventListener('change', ({ window }) => {
  breakpointStore.set(bucketFor(window.width))
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
