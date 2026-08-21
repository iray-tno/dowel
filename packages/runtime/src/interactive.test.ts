// `disabled` means five things at once, and this is where they agree.
//
// The bug this exists over: the compiled path announced `aria-disabled`
// and then ran the handler anyway, and once keyboard activation existed it
// ran on Enter and Space too. The fallback component suppressed the click
// and nothing else. One prop, two paths, two different wrong answers --
// which is what happens when a rule is spelled out separately in each
// place that needs it.

import assert from 'node:assert/strict'
import { test } from 'node:test'

import { hozoActivateKeyDown, hozoActivateKeyUp } from './activate.ts'
import { hozoInteractive } from './interactive.ts'

const press = () => {}

test('an enabled control is operable by pointer and keyboard', () => {
  const props = hozoInteractive(press)
  assert.equal(props.onClick, press)
  assert.equal(props.onKeyDown, hozoActivateKeyDown)
  assert.equal(props.onKeyUp, hozoActivateKeyUp)
  assert.equal(props.tabIndex, 0)
  // Never disabled means nothing said about it, rather than
  // `aria-disabled="false"` on every control in the page.
  assert.equal(props['aria-disabled'], undefined)
})

test('a disabled control is operable by neither', () => {
  const props = hozoInteractive(press, true)
  assert.equal(props.onClick, undefined)
  assert.equal(props.onKeyDown, undefined)
  assert.equal(props.onKeyUp, undefined)
  assert.equal(props['aria-disabled'], true)
})

test('the styling hook is a presence attribute, not a boolean', () => {
  // React renders `data-x={false}` as the string "false" -- unlike
  // `aria-*`, unlike `disabled` -- and `[data-hozo-disabled]` matches on
  // presence, so a boolean here would leave every control looking
  // permanently disabled. `undefined` is the only value React omits.
  assert.equal(hozoInteractive(press, true)['data-hozo-disabled'], '')
  assert.equal(hozoInteractive(press)['data-hozo-disabled'], undefined)
})

test('a disabled control leaves the tab order but stays reachable by focus()', () => {
  // `-1`, not absent: out of the tab order is the decision, while
  // `element.focus()` still works, which focus management and roving
  // tabindex both need. docs/decisions/001, rule 1a.
  assert.equal(hozoInteractive(press, true).tabIndex, -1)
})

test('the two branches carry the same keys', () => {
  // Otherwise spreading this leaves a stale attribute behind from the
  // previous render when the flag flips.
  assert.deepEqual(
    Object.keys(hozoInteractive(press)).sort(),
    Object.keys(hozoInteractive(press, true)).sort(),
  )
})

test('any truthy value disables, since the guard is the author expression', () => {
  // The compiler passes the author's expression through verbatim -- it is
  // not required to be a boolean, and `disabled={items.length}` is
  // something people write.
  for (const value of [true, 1, 'yes', {}, []]) {
    assert.equal(hozoInteractive(press, value).onClick, undefined, `${String(value)} did not disable`)
  }
  for (const value of [false, 0, '', null, undefined, Number.NaN]) {
    assert.equal(hozoInteractive(press, value).onClick, press, `${String(value)} disabled`)
  }
})
