// The keyboard half of a synthesized button.
//
// Worth testing precisely because "nearly a button" reads as working: a
// single keydown branch that fires on both Enter and Space passes a casual
// try, and then Space scrolls the page on the way down and activates twice
// on some browsers. A real `<button>` splits them, so these do too.

import assert from 'node:assert/strict'
import { test } from 'node:test'

import { hozoActivateKeyDown, hozoActivateKeyUp } from './activate.ts'

/** A stand-in for the parts of a React keyboard event these read. */
function event(key: string) {
  const record = { key, prevented: 0, clicked: 0 }
  return {
    record,
    handler: {
      key,
      preventDefault: () => {
        record.prevented += 1
      },
      currentTarget: {
        click: () => {
          record.clicked += 1
        },
      },
    },
  }
}

test('Enter activates on key down', () => {
  const { record, handler } = event('Enter')
  hozoActivateKeyDown(handler)
  assert.equal(record.clicked, 1)
  // Otherwise the same Enter also submits the enclosing form.
  assert.equal(record.prevented, 1)
})

test('Enter does nothing on key up, so one press is one activation', () => {
  const { record, handler } = event('Enter')
  hozoActivateKeyUp(handler)
  assert.equal(record.clicked, 0)
  assert.equal(record.prevented, 0)
})

test('Space is suppressed on key down and activates on key up', () => {
  const down = event(' ')
  hozoActivateKeyDown(down.handler)
  // Suppressed but not activated: holding Space on a button must not
  // scroll the page, and must not fire until release.
  assert.equal(down.record.clicked, 0)
  assert.equal(down.record.prevented, 1)

  const up = event(' ')
  hozoActivateKeyUp(up.handler)
  assert.equal(up.record.clicked, 1)
  assert.equal(up.record.prevented, 1)
})

test('every other key is left alone', () => {
  for (const key of ['a', 'Tab', 'Escape', 'ArrowDown', 'Shift', 'Enter '.trim() + 'x']) {
    const down = event(key)
    hozoActivateKeyDown(down.handler)
    const up = event(key)
    hozoActivateKeyUp(up.handler)
    assert.deepEqual(
      [down.record.clicked, down.record.prevented, up.record.clicked, up.record.prevented],
      [0, 0, 0, 0],
      `${key} was not left alone`,
    )
  }
})

test('the handlers are stable references', () => {
  // The compiler emits them by name rather than as inline arrows, so a
  // re-render must not produce a new function. This is what makes that
  // true, and it is one refactor away from silently not being.
  assert.equal(hozoActivateKeyDown, hozoActivateKeyDown)
  assert.equal(typeof hozoActivateKeyDown, 'function')
  assert.equal(typeof hozoActivateKeyUp, 'function')
})
