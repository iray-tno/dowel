// Where focus goes when a dialog opens, and where it returns when one
// closes.
//
// Split out from the platform glue and kept free of `react` and `document`
// imports so the rules can be tested at all. The rest of a dialog's
// behaviour -- the trap, the inert background, Escape -- is delegated to
// the platform rather than reimplemented (see `./dialog.tsx`), so these
// two rules are the whole of what Dowel decides.

/** The subset of an element the focus rules actually look at. */
export interface FocusCandidate {
  /** Whether the author asked for this one specifically. */
  autofocus?: boolean
  /** Whether it can hold focus at all: not disabled, not hidden. */
  focusable?: boolean
}

/**
 * Which candidate should receive focus when the dialog opens, as an index,
 * or `null` to focus the dialog itself.
 *
 * The order is: an explicit `autofocus` first, then the first focusable
 * thing, then nothing.
 *
 * Focusing the dialog rather than the first control is the right fallback
 * and not a giving-up: a screen reader announces the dialog's name and role
 * from there, which is what tells someone what just happened. Landing on
 * the first text field instead announces the field and leaves the reason
 * for it unsaid.
 *
 * The author's `autofocus` wins over document order even when it is a
 * later element, because it is the one place someone has said what the
 * dialog is *for* -- a confirm button, a search box.
 */
export function initialFocusIndex(candidates: readonly FocusCandidate[]): number | null {
  const requested = candidates.findIndex((c) => c.autofocus && c.focusable)
  if (requested !== -1) return requested
  const first = candidates.findIndex((c) => c.focusable)
  return first === -1 ? null : first
}

/**
 * Whether focus should be put back on `opener` when the dialog closes.
 *
 * Not unconditional: if the opener is gone from the page, or can no longer
 * take focus, restoring to it moves focus to the document body and the
 * reading position is lost silently. Saying no here lets the caller leave
 * focus where the platform put it, which is recoverable.
 *
 * The case this exists for is the common one -- a dialog whose confirm
 * action removes the row its own trigger lived in.
 */
export function shouldRestoreFocus(opener: FocusCandidate | null | undefined): boolean {
  return opener?.focusable === true
}
