import assert from 'node:assert/strict'
import test from 'node:test'

import { dowelImageSource } from './image-source.native.ts'

test('a URI becomes React Native source metadata', () => {
  assert.deepEqual(dowelImageSource('https://example.com/image.png'), { uri: 'https://example.com/image.png' })
})

test('a Metro local asset id is retained', () => {
  assert.equal(dowelImageSource(42), 42)
})

test('an advanced Native source object is retained', () => {
  const source = { uri: 'file:///cached.png', width: 20, height: 20 }
  assert.equal(dowelImageSource(source), source)
})
