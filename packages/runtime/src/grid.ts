export type GridTrack =
  | { kind: 'fr'; value: number }
  | { kind: 'points'; value: number }
  | { kind: 'minmax'; min: number; value: number }

export interface GridCell {
  child: number | null
  tracks: readonly GridTrack[]
}

export interface GridItemPlacement {
  span: number
  /** Zero-based explicit column; absent means normal row auto-placement. */
  columnStart?: number
  rowSpan?: number
  /** Zero-based explicit row; absent means normal row auto-placement. */
  rowStart?: number
}

export interface PlacedGridItem {
  child: number
  column: number
  columnSpan: number
  row: number
  rowSpan: number
}

export function gridLayout(items: readonly GridItemPlacement[], columnCount: number): PlacedGridItem[] {
  if (columnCount <= 0) return []
  const occupied: boolean[][] = []
  const placed: PlacedGridItem[] = []
  let cursorRow = 0
  let cursorColumn = 0
  const free = (row: number, column: number, width: number, height: number) => {
    for (let y = row; y < row + height; y += 1) {
      for (let x = column; x < column + width; x += 1) {
        if (occupied[y]?.[x]) return false
      }
    }
    return true
  }

  items.forEach((item, child) => {
    const columnSpan = Math.min(columnCount, Math.max(1, Math.trunc(item.span)))
    const rowSpan = Math.max(1, Math.trunc(item.rowSpan ?? 1))
    let row = Math.max(0, Math.trunc(item.rowStart ?? cursorRow))
    let column = item.columnStart === undefined
      ? (item.rowStart === undefined ? cursorColumn : 0)
      : Math.min(columnCount - columnSpan, Math.max(0, Math.trunc(item.columnStart)))

    while (column + columnSpan > columnCount || !free(row, column, columnSpan, rowSpan)) {
      if (item.columnStart !== undefined) {
        row += 1
      } else {
        column += 1
        if (column + columnSpan > columnCount) {
          row += 1
          column = 0
        }
      }
    }
    for (let y = row; y < row + rowSpan; y += 1) {
      const cells = (occupied[y] ??= [])
      for (let x = column; x < column + columnSpan; x += 1) cells[x] = true
    }
    placed.push({ child, column, columnSpan, row, rowSpan })
    cursorRow = row
    cursorColumn = column + columnSpan
    if (cursorColumn >= columnCount) {
      cursorRow += 1
      cursorColumn = 0
    }
  })
  return placed
}

/** Pure row auto-placement. Explicit coordinates/dense can replace this step later. */
export function gridRows(items: readonly GridItemPlacement[], tracks: readonly GridTrack[]): GridCell[][] {
  if (tracks.length === 0 || items.length === 0) return []
  const layout = gridLayout(items, tracks.length)
  const rowCount = Math.max(0, ...layout.map((item) => item.row + item.rowSpan))
  const rows: GridCell[][] = Array.from({ length: rowCount }, () => [])
  for (let row = 0; row < rowCount; row += 1) {
    // Read once: `rows` was built with exactly `rowCount` entries, so this
    // index is in range by construction.
    const cells = rows[row]!
    let column = 0
    const starts = layout.filter((item) => item.row === row).sort((a, b) => a.column - b.column)
    for (const item of starts) {
      while (column < item.column) {
        cells.push({ child: null, tracks: tracks.slice(column, column + 1) })
        column += 1
      }
      cells.push({ child: item.child, tracks: tracks.slice(column, column + item.columnSpan) })
      column += item.columnSpan
    }
    while (column < tracks.length) {
      cells.push({ child: null, tracks: tracks.slice(column, column + 1) })
      column += 1
    }
  }
  return rows
}

export function gridCellStyle(tracks: readonly GridTrack[], internalGap = 0): Record<string, number> {
  const fr = tracks.reduce(
    (sum, track) => sum + (track.kind === 'fr' || track.kind === 'minmax' ? track.value : 0),
    0,
  )
  const points = tracks.reduce(
    (sum, track) => sum + (track.kind === 'points' ? track.value : track.kind === 'minmax' ? track.min : 0),
    Math.max(0, tracks.length - 1) * internalGap,
  )
  return { flexBasis: points, flexGrow: fr, flexShrink: fr > 0 ? 1 : 0 }
}

export function gridTrackSizes(tracks: readonly GridTrack[], width: number, gap: number): number[] {
  const fixed = tracks.reduce(
    (sum, track) => sum + (track.kind === 'points' ? track.value : track.kind === 'minmax' ? track.min : 0),
    0,
  )
  const fr = tracks.reduce(
    (sum, track) => sum + (track.kind === 'fr' || track.kind === 'minmax' ? track.value : 0),
    0,
  )
  const free = Math.max(0, width - fixed - Math.max(0, tracks.length - 1) * gap)
  return tracks.map((track) => {
    if (track.kind === 'points') return track.value
    const share = fr === 0 ? 0 : free * track.value / fr
    return track.kind === 'minmax' ? track.min + share : share
  })
}

export function gridRowSizes(
  layout: readonly PlacedGridItem[],
  measuredHeights: readonly number[],
  rowGap: number,
  explicitTracks: readonly GridTrack[] = [],
): number[] {
  const count = Math.max(explicitTracks.length, 0, ...layout.map((item) => item.row + item.rowSpan))
  const rows = Array.from({ length: count }, (_, row) =>
    explicitTracks[row]?.kind === 'points'
      ? explicitTracks[row].value
      : explicitTracks[row]?.kind === 'minmax'
        ? explicitTracks[row].min
        : 0)
  for (const item of layout.filter((item) => item.rowSpan === 1)) {
    const track = explicitTracks[item.row]
    if (track?.kind === 'points') continue
    const factor = track?.kind === 'fr' || track?.kind === 'minmax' ? track.value : 1
    const intrinsic = Math.max(0, (measuredHeights[item.child] ?? 0) - (track?.kind === 'minmax' ? track.min : 0))
    const unit = intrinsic / factor
    if (track?.kind === 'fr' || track?.kind === 'minmax') {
      for (let row = 0; row < explicitTracks.length; row += 1) {
        const candidate = explicitTracks[row]!
        if (candidate.kind === 'fr') rows[row] = Math.max(rows[row]!, unit * candidate.value)
        if (candidate.kind === 'minmax') {
          rows[row] = Math.max(rows[row]!, candidate.min + unit * candidate.value)
        }
      }
    } else {
      rows[item.row] = Math.max(rows[item.row] ?? 0, measuredHeights[item.child] ?? 0)
    }
  }
  for (const item of [...layout].sort((a, b) => a.rowSpan - b.rowSpan)) {
    if (item.rowSpan === 1) continue
    const current = rows.slice(item.row, item.row + item.rowSpan).reduce((a, b) => a + b, 0)
      + (item.rowSpan - 1) * rowGap
    const deficit = Math.max(0, (measuredHeights[item.child] ?? 0) - current)
    const flexible = Array.from({ length: item.rowSpan }, (_, offset) => item.row + offset)
      .filter((row) => explicitTracks[row]?.kind !== 'points')
    const targets = flexible.length > 0 ? flexible : []
    for (const row of targets) {
      rows[row] = (rows[row] ?? 0) + deficit / targets.length
    }
  }
  return rows
}
