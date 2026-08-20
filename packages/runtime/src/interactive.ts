// Everything `disabled` means, in one place.
//
// `docs/decisions/001-disabled-and-focus.md` settles it as five things at
// once: inoperable, announced as disabled, present in the accessibility
// tree, out of the tab order, and -- for an element Hozo made interactive
// itself -- not activatable by Enter or Space either.
//
// Five things emitted as five independent expressions is exactly how they
// come apart, and they had. The compiled path announced `aria-disabled`
// and then ran the handler anyway; the fallback component in `@hozo/core`
// suppressed the handler but had no keyboard activation to suppress. Two
// paths, two different wrong answers, from one prop.
//
// So both call this instead. A drift between them now needs someone to
// edit this file and mean it.
//
// Only for elements Hozo had to synthesize -- a `<div role="button">`. A
// real `<button>` takes the `disabled` attribute and the browser does all
// five, which is why `Button` does not come through here.

import { hozoActivateKeyDown, hozoActivateKeyUp } from './activate.ts'

/** What React hands a click handler on the element Hozo synthesized. */
type PressHandler = (event: never) => void

/**
 * The props that make an element a control, and unmake it when disabled.
 *
 * `tabIndex` is `-1` rather than absent when disabled: it leaves the tab
 * order, which is the decision, while staying reachable by `focus()`,
 * which is what focus management and roving tabindex need. See 001's
 * rule 1a.
 *
 * Both branches carry the same keys so that spreading this never leaves a
 * stale attribute behind from the previous render.
 */
export function hozoInteractive(onPress?: PressHandler, disabled?: unknown) {
  return disabled
    ? {
        'aria-disabled': true,
        tabIndex: -1,
        onClick: undefined,
        onKeyDown: undefined,
        onKeyUp: undefined,
      }
    : {
        // Not `false`: an element that was never disabled says nothing
        // about it, and `aria-disabled="false"` on every control in a page
        // is noise a screen reader has to read past.
        'aria-disabled': undefined,
        tabIndex: 0,
        onClick: onPress,
        onKeyDown: hozoActivateKeyDown,
        onKeyUp: hozoActivateKeyUp,
      }
}
