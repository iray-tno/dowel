import assert from 'node:assert/strict'
import { test } from 'node:test'

import { gridRows, gridTrackStyle, type GridTrack } from './grid.ts'

const tracks: GridTrack[] = [
  { kind: 'points', value: 120 },
  { kind: 'fr', value: 2 },
  { kind: 'fr', value: 1 },
]

test('auto-placement fills rows and preserves empty final tracks', () => {
  assert.deepEqual(gridRows(4, tracks).map((row) => row.map((cell) => cell.child)), [
    [0, 1, 2],
    [3, null, null],
  ])
})

test('track styles distinguish fixed space from proportional remainder', () => {
  assert.deepEqual(gridTrackStyle(tracks[0]), { flexBasis: 120, flexGrow: 0, flexShrink: 0 })
  assert.deepEqual(gridTrackStyle(tracks[1]), { flexBasis: 0, flexGrow: 2, flexShrink: 1 })
})
