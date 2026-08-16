import assert from 'node:assert/strict'
import { test } from 'node:test'

import { typeCheckStyles } from './typecheck.ts'

test('accepts a style React Native would accept', () => {
  assert.deepEqual(
    typeCheckStyles([{ candidate: 'p-4', style: '{ paddingTop: 16, flexDirection: "row" }' }]),
    [],
  )
})

test('rejects a key React Native does not have', () => {
  // The check that makes a clean run mean something. Without it, "0 errors"
  // is equally consistent with the checker never having run -- which is
  // exactly how the keyword-collision guard managed to report a clean table
  // while being unable to see collisions.
  const errors = typeCheckStyles([{ candidate: 'made-up', style: '{ gridTemplateRows: 3 }' }])
  assert.equal(errors.length, 1)
  assert.equal(errors[0].candidate, 'made-up')
})

test('rejects a value of the wrong type for a real key', () => {
  // The half a property-name check can't do. `opacity` exists and takes a
  // number, so a CSS-shaped string is the error a name-only check misses.
  const errors = typeCheckStyles([{ candidate: 'opacity-50', style: '{ opacity: "50%" }' }])
  assert.equal(errors.length, 1)

  // Not the example that first came to mind: React Native 0.87 accepts
  // `fontWeight` as a string *and* as a number, so `700` is valid there.
  // Worth pinning, since Dowel emits the string form on the strength of a
  // comment that says RN requires it.
  assert.deepEqual(typeCheckStyles([{ candidate: 'font-bold', style: '{ fontWeight: 700 }' }]), [])
})

test('a style valid on one component is accepted, not intersected', () => {
  // `overflow: 'scroll'` is fine on a View and absent from ImageStyle. The
  // question is "could React Native accept this", so the three style types
  // are a union -- an intersection would reject correct output.
  assert.deepEqual(
    typeCheckStyles([{ candidate: 'overflow-scroll', style: '{ overflow: "scroll" }' }]),
    [],
  )
})

test('attributes each error to the candidate that produced it', () => {
  const errors = typeCheckStyles([
    { candidate: 'fine', style: '{ opacity: 0.5 }' },
    { candidate: 'broken', style: '{ nonsense: 1 }' },
    { candidate: 'also-fine', style: '{ margin: 4 }' },
  ])
  assert.equal(errors.length, 1)
  assert.equal(errors[0].candidate, 'broken')
})
