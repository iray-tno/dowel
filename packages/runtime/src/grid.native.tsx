import { Children, isValidElement, type ReactNode } from 'react'
import { View } from 'react-native'

import { gridCellStyle, gridRows, type GridTrack } from './grid.ts'

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
  const spans = list.map((child) =>
    isValidElement<{ columnSpan?: number }>(child) && child.type === DowelGridItem
      ? child.props.columnSpan ?? 1
      : 1,
  )
  const rows = gridRows(spans, tracks)

  return rows.map((cells, row) => (
    <View key={row} style={{ flexDirection: 'row', columnGap }}>
      {cells.map((cell, column) => (
        <View key={column} style={gridCellStyle(cell.tracks, columnGap)}>
          {cell.child === null ? null : unwrapGridItem(list[cell.child])}
        </View>
      ))}
    </View>
  ))
}

interface ItemProps {
  columnSpan?: number
  children?: ReactNode
}

/** Compiler marker consumed by DowelGrid; outside one it is an identity wrapper. */
export function DowelGridItem({ children }: ItemProps): ReactNode {
  return children
}

function unwrapGridItem(child: ReactNode): ReactNode {
  return isValidElement<ItemProps>(child) && child.type === DowelGridItem
    ? child.props.children
    : child
}
