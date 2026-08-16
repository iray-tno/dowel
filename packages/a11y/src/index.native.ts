// The React Native entry point for `@dowel/a11y`. Metro resolves a platform
// extension ahead of the plain file, so `./dialog.native.tsx` is what an app
// gets on device. See `@dowel/runtime`'s `index.native.ts` for the same
// arrangement and why it is needed.

export { initialFocusIndex, shouldRestoreFocus, type FocusCandidate } from './focus.ts'
export { DowelDialog, type DowelDialogProps } from './dialog.native.tsx'
