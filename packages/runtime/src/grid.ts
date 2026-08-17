export type GridTrack =
  | { kind: 'fr'; value: number }
  | { kind: 'points'; value: number }

export interface GridCell {
  child: number | null
  tracks: readonly GridTrack[]
}

export interface GridItemPlacement {
  span: number
  /** Zero-based explicit column; absent means normal row auto-placement. */
  columnStart?: number
}

/** Pure row auto-placement. Explicit coordinates/dense can replace this step later. */
export function gridRows(items: readonly GridItemPlacement[], tracks: readonly GridTrack[]): GridCell[][] {
  if (tracks.length === 0 || items.length === 0) return []
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

  items.forEach((item, child) => {
    const span = Math.min(tracks.length, Math.max(1, Math.trunc(item.span)))
    const requestedStart = item.columnStart === undefined
      ? column
      : Math.min(tracks.length - 1, Math.max(0, Math.trunc(item.columnStart)))
    if (column > requestedStart || requestedStart + span > tracks.length) finishRow()
    const start = item.columnStart === undefined ? column : requestedStart
    while (column < start) {
      row.push({ child: null, tracks: tracks.slice(column, column + 1) })
      column += 1
    }
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
