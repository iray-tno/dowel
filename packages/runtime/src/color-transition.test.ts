import assert from 'node:assert/strict'
import test from 'node:test'

import { blendColor } from './color-transition.ts'

test('blends Tailwind hex colours at an interrupted transition point', () => {
  assert.equal(blendColor('#000', '#ffffff', 0.5), 'rgba(128, 128, 128, 1)')
})

test('fades from a transparent background without inventing an opaque start', () => {
  assert.equal(blendColor('transparent', '#ff0000', 0.25), 'rgba(64, 0, 0, 0.25)')
})

test('keeps unknown native colour syntax usable at the endpoints', () => {
  assert.equal(blendColor('navy', 'tomato', 0), 'navy')
  assert.equal(blendColor('navy', 'tomato', 1), 'tomato')
})
