import assert from 'node:assert/strict'
import { test } from 'node:test'

import { gridCellStyle, gridLayout, gridRows, gridRowSizes, gridTrackSizes, type GridTrack } from './grid.ts'

const tracks: GridTrack[] = [
  { kind: 'points', value: 120 },
  { kind: 'fr', value: 2 },
  { kind: 'fr', value: 1 },
]

test('auto-placement fills rows and preserves empty final tracks', () => {
  const items = [1, 1, 1, 1].map((span) => ({ span }))
  assert.deepEqual(gridRows(items, tracks).map((row) => row.map((cell) => cell.child)), [
    [0, 1, 2],
    [3, null, null],
  ])
})

test('cell styles distinguish fixed space from proportional remainder', () => {
  assert.deepEqual(gridCellStyle([tracks[0]]), { flexBasis: 120, flexGrow: 0, flexShrink: 0 })
  assert.deepEqual(gridCellStyle([tracks[1]]), { flexBasis: 0, flexGrow: 2, flexShrink: 1 })
  assert.deepEqual(gridCellStyle(tracks.slice(0, 2), 16), {
    flexBasis: 136,
    flexGrow: 2,
    flexShrink: 1,
  })
})

test('minmax tracks reserve their floor before distributing fractions', () => {
  const minmax: GridTrack[] = [
    { kind: 'minmax', min: 120, value: 2 },
    { kind: 'fr', value: 1 },
  ]
  assert.deepEqual(gridCellStyle(minmax.slice(0, 1)), {
    flexBasis: 120,
    flexGrow: 2,
    flexShrink: 1,
  })
  assert.deepEqual(gridTrackSizes(minmax, 420, 0), [320, 100])
})

test('a span moves to the next row when it cannot fit', () => {
  const items = [2, 2, 1].map((span) => ({ span }))
  assert.deepEqual(gridRows(items, tracks).map((row) => row.map((cell) => cell.child)), [
    [0, null],
    [1, 2],
  ])
})

test('an explicit column leaves empty tracks and never backfills an earlier row', () => {
  const items = [{ span: 1, columnStart: 1 }, { span: 2, columnStart: 0 }]
  assert.deepEqual(gridRows(items, tracks).map((row) => row.map((cell) => cell.child)), [
    [null, 0, null],
    [1, null],
  ])
})

test('row spans reserve a two-dimensional rectangle from later items', () => {
  const layout = gridLayout([{ span: 1, rowSpan: 2 }, { span: 2 }, { span: 1 }], 3)
  assert.deepEqual(layout, [
    { child: 0, column: 0, columnSpan: 1, row: 0, rowSpan: 2 },
    { child: 1, column: 1, columnSpan: 2, row: 0, rowSpan: 1 },
    { child: 2, column: 1, columnSpan: 1, row: 1, rowSpan: 1 },
  ])
  assert.deepEqual(gridRowSizes(layout, [70, 20, 30], 10), [25, 35])
})

test('explicit fr rows share one flex fraction under intrinsic height', () => {
  const layout = gridLayout([{ span: 1, rowStart: 0 }, { span: 1, rowStart: 1 }], 2)
  const rows: GridTrack[] = [{ kind: 'fr', value: 1 }, { kind: 'fr', value: 1 }, { kind: 'fr', value: 1 }]
  assert.deepEqual(gridRowSizes(layout, [10, 30], 0, rows), [30, 30, 30])
})
