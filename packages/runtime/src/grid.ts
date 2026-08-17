export type GridTrack =
  | { kind: 'fr'; value: number }
  | { kind: 'points'; value: number }

export interface GridCell {
  child: number | null
  tracks: readonly GridTrack[]
}

/** Pure row auto-placement. Explicit coordinates/dense can replace this step later. */
export function gridRows(spans: readonly number[], tracks: readonly GridTrack[]): GridCell[][] {
  if (tracks.length === 0 || spans.length === 0) return []
  const rows: GridCell[][] = []
  let row: GridCell[] = []
  let column = 0

  const finishRow = () => {
    while (column < tracks.length) {
      row.push({ child: null, tracks: tracks.slice(column, column + 1) })
      column += 1
    }
    rows.push(row)
    row = []
    column = 0
  }

  spans.forEach((requestedSpan, child) => {
    const span = Math.min(tracks.length, Math.max(1, Math.trunc(requestedSpan)))
    if (column > 0 && column + span > tracks.length) finishRow()
    row.push({ child, tracks: tracks.slice(column, column + span) })
    column += span
    if (column === tracks.length) finishRow()
  })
  if (row.length > 0) finishRow()
  return rows
}

export function gridCellStyle(tracks: readonly GridTrack[], internalGap = 0): Record<string, number> {
  const fr = tracks.reduce((sum, track) => sum + (track.kind === 'fr' ? track.value : 0), 0)
  const points = tracks.reduce(
    (sum, track) => sum + (track.kind === 'points' ? track.value : 0),
    Math.max(0, tracks.length - 1) * internalGap,
  )
  return { flexBasis: points, flexGrow: fr, flexShrink: fr > 0 ? 1 : 0 }
}
