import { Children, type ReactNode } from 'react'
import { View } from 'react-native'

import { gridRows, gridTrackStyle, type GridTrack } from './grid.ts'

interface Props {
  tracks: readonly GridTrack[]
  columnGap?: number
  children?: ReactNode
}

/**
 * The first renderer for Dowel's grid solver boundary. It needs no
 * measurement: fixed tracks and fr tracks are solved by one Yoga flex row.
 * Empty cells preserve track widths on the final row.
 */
export function DowelGrid({ tracks, columnGap = 0, children }: Props): ReactNode {
  const list = Children.toArray(children)
  const rows = gridRows(list.length, tracks)

  return rows.map((cells, row) => (
    <View key={row} style={{ flexDirection: 'row', columnGap }}>
      {cells.map((cell, column) => (
        <View key={column} style={gridTrackStyle(cell.track)}>
          {cell.child === null ? null : list[cell.child]}
        </View>
      ))}
    </View>
  ))
}
