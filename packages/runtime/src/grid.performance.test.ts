import assert from 'node:assert/strict'
import { performance } from 'node:perf_hooks'
import test from 'node:test'

import { gridLayout } from './grid.ts'

test('ordinary grid auto-placement keeps a wide performance margin', () => {
  const items = Array.from({ length: 10_000 }, () => ({ span: 1 }))
  const started = performance.now()
  const layout = gridLayout(items, 12)
  const elapsed = performance.now() - started

  assert.equal(layout.length, items.length)
  assert.deepEqual(layout.at(-1), {
    child: 9_999,
    column: 3,
    columnSpan: 1,
    row: 833,
    rowSpan: 1,
  })
  // Deliberately generous for shared CI. This is a regression tripwire for
  // accidentally turning the measurement-free path quadratic, not a claim
  // that every device completes JavaScript work in the same wall time.
  assert.ok(elapsed < 500, `10k-item grid layout took ${elapsed.toFixed(1)}ms`)
})
