import assert from 'node:assert/strict'
import { test } from 'node:test'

import { gridCellStyle, gridRows, type GridTrack } from './grid.ts'

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
