import assert from 'node:assert/strict'
import { test } from 'node:test'

import { spacingTargets } from './spacing.ts'

/// Stand-in for `React.isValidElement`: objects are elements, strings are
/// text nodes. Enough to exercise the rule, and keeps the test react-free.
const isElement = (child: unknown) => typeof child === 'object' && child !== null

test('spaces every element but the last', () => {
  const children = [{ el: 1 }, { el: 2 }, { el: 3 }]
  assert.deepEqual(spacingTargets(children, isElement), [0, 1])
})

test('a single child gets nothing', () => {
  // It is `:last-child` on Web, so the rule matches nothing there either.
  assert.deepEqual(spacingTargets([{ el: 1 }], isElement), [])
  assert.deepEqual(spacingTargets([], isElement), [])
})

test('a trailing text node does not take the last slot', () => {
  // The case this function exists for. `:not(:last-child)` selects
  // elements, so the final <Text> is still last on Web even with a stray
  // string after it -- taking "all but the final array entry" would space
  // it and lay out differently from the same source.
  const children = [{ el: 1 }, { el: 2 }, 'trailing text']
  assert.deepEqual(spacingTargets(children, isElement), [0])
})

test('text between elements is skipped without shifting the answer', () => {
  const children = [{ el: 1 }, 'gap text', { el: 2 }]
  assert.deepEqual(spacingTargets(children, isElement), [0])
})

test('returns indices into the original array, not into the elements', () => {
  // The caller maps over the full list, so an index that counted only
  // elements would style the wrong child.
  const children = ['lead', { el: 1 }, { el: 2 }, { el: 3 }]
  assert.deepEqual(spacingTargets(children, isElement), [1, 2])
})
