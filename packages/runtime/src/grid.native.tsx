import { Children, isValidElement, useState, type ReactNode } from 'react'
import { View, type LayoutChangeEvent } from 'react-native'

import {
  gridCellStyle,
  gridLayout,
  gridRows,
  gridRowSizes,
  gridTrackSizes,
  type GridTrack,
} from './grid.ts'

interface Props {
  tracks: readonly GridTrack[]
  columnGap?: number
  rowGap?: number
  children?: ReactNode
}

/**
 * The first renderer for Dowel's grid solver boundary. It needs no
 * measurement: fixed tracks and fr tracks are solved by one Yoga flex row.
 * Empty cells preserve track widths on the final row.
 */
export function DowelGrid({ tracks, columnGap = 0, rowGap = 0, children }: Props): ReactNode {
  const [width, setWidth] = useState(0)
  const [heights, setHeights] = useState<number[]>([])
  const list = Children.toArray(children)
  const placements = list.map((child) =>
    isValidElement<ItemProps>(child) && child.type === DowelGridItem
      ? {
          span: child.props.columnSpan ?? 1,
          columnStart: child.props.columnStart,
          rowSpan: child.props.rowSpan,
          rowStart: child.props.rowStart,
        }
      : { span: 1 },
  )
  const layout = gridLayout(placements, tracks.length)
  const measured = placements.some((item) => (item.rowSpan ?? 1) > 1 || item.rowStart !== undefined)
  if (measured) {
    const columns = gridTrackSizes(tracks, width, columnGap)
    const rows = gridRowSizes(layout, heights, rowGap)
    const offsets = (sizes: readonly number[], gap: number) =>
      sizes.map((_, index) => sizes.slice(0, index).reduce((a, b) => a + b, 0) + index * gap)
    const left = offsets(columns, columnGap)
    const top = offsets(rows, rowGap)
    const totalHeight = rows.reduce((a, b) => a + b, 0) + Math.max(0, rows.length - 1) * rowGap
    const rememberHeight = (child: number) => (event: LayoutChangeEvent) => {
      const height = event.nativeEvent.layout.height
      setHeights((current) => current[child] === height
        ? current
        : Object.assign([...current], { [child]: height }))
    }
    return (
      <View
        style={{ position: 'relative', alignSelf: 'stretch', height: totalHeight }}
        onLayout={(event) => setWidth(event.nativeEvent.layout.width)}
      >
        {layout.map((item) => {
          const cellWidth = columns.slice(item.column, item.column + item.columnSpan)
            .reduce((a, b) => a + b, 0) + (item.columnSpan - 1) * columnGap
          const cellHeight = rows.slice(item.row, item.row + item.rowSpan)
            .reduce((a, b) => a + b, 0) + (item.rowSpan - 1) * rowGap
          return (
            <View
              key={item.child}
              onLayout={rememberHeight(item.child)}
              style={{
                position: 'absolute',
                left: left[item.column],
                top: top[item.row],
                width: cellWidth,
                ...(cellHeight > 0 ? { height: cellHeight } : {}),
              }}
            >
              {unwrapGridItem(list[item.child])}
            </View>
          )
        })}
      </View>
    )
  }
  const rows = gridRows(placements, tracks)

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
  columnStart?: number
  rowSpan?: number
  rowStart?: number
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
