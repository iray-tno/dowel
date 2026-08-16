import assert from 'node:assert/strict'
import { test } from 'node:test'

import { initialFocusIndex, shouldRestoreFocus } from './focus.ts'

const focusable = (autofocus = false) => ({ focusable: true, autofocus })
const inert = { focusable: false }

test('focuses the first focusable thing', () => {
  assert.equal(initialFocusIndex([inert, focusable(), focusable()]), 1)
})

test('an explicit autofocus wins over document order', () => {
  // The one place someone has said what the dialog is *for* -- a confirm
  // button, a search box -- so it beats "whatever came first".
  assert.equal(initialFocusIndex([focusable(), focusable(true), focusable()]), 1)
})

test('an autofocus that cannot take focus is ignored, not obeyed', () => {
  // A disabled confirm button is the ordinary case: the author's intent is
  // clear and unreachable, so fall through rather than focusing nothing.
  assert.equal(initialFocusIndex([{ focusable: false, autofocus: true }, focusable()]), 1)
})

test('with nothing focusable, focus goes to the dialog itself', () => {
  // Not a failure: a screen reader announces the dialog's name and role
  // from there, which is what tells someone what just happened. Landing on
  // a control instead announces the control and leaves the reason unsaid.
  assert.equal(initialFocusIndex([inert, inert]), null)
  assert.equal(initialFocusIndex([]), null)
})

test('focus returns to the opener only while the opener can hold it', () => {
  assert.equal(shouldRestoreFocus(focusable()), true)
  // The case this exists for: a dialog whose confirm action removes the row
  // its own trigger lived in. Restoring to a detached element drops focus
  // to the body and loses the reading position silently.
  assert.equal(shouldRestoreFocus(inert), false)
  assert.equal(shouldRestoreFocus(null), false)
  assert.equal(shouldRestoreFocus(undefined), false)
})
