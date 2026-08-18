// Accessibility behaviour that can only exist at runtime (proposal §10.3):
// focus, keyboard, and the modal semantics a compiler can't decide.
//
// The rules Hozo actually decides live in `./focus.ts`, free of `react`
// and `document` so they can be tested. Everything else is delegated to the
// platform -- see `./dialog.tsx` for why that is the design and not a
// shortcut.

export { initialFocusIndex, shouldRestoreFocus, type FocusCandidate } from './focus.ts'
export { HozoDialog, type HozoDialogProps } from './dialog.tsx'
