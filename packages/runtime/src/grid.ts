export type GridTrack =
  | { kind: 'fr'; value: number }
  | { kind: 'points'; value: number }

export interface GridCell {
  child: number | null
  track: GridTrack
}

/** Pure auto-placement core. Later placement/span support can replace only this step. */
export function gridRows(childCount: number, tracks: readonly GridTrack[]): GridCell[][] {
  if (tracks.length === 0 || childCount === 0) return []
  const rowCount = Math.ceil(childCount / tracks.length)
  return Array.from({ length: rowCount }, (_, row) =>
    tracks.map((track, column) => {
      const child = row * tracks.length + column
      return { child: child < childCount ? child : null, track }
    }),
  )
}

export function gridTrackStyle(track: GridTrack): Record<string, number> {
  return track.kind === 'fr'
    ? { flexBasis: 0, flexGrow: track.value, flexShrink: 1 }
    : { flexBasis: track.value, flexGrow: 0, flexShrink: 0 }
}
