import { useCallback, useState } from 'react'
import {
  Pressable,
  type MouseEvent,
  type NativeSyntheticEvent,
  type PressableProps,
  type PressableStateCallbackType,
  type StyleProp,
  type TargetedEvent,
  type ViewStyle,
} from 'react-native'

export interface DowelPressableState extends PressableStateCallbackType {
  hovered: boolean
  focused: boolean
}

export interface DowelPressableProps extends Omit<PressableProps, 'style'> {
  style?: StyleProp<ViewStyle> | ((state: DowelPressableState) => StyleProp<ViewStyle>)
}

const HOVERED = 1
const FOCUSED = 2

export function DowelPressable({
  style,
  onHoverIn,
  onHoverOut,
  onFocus,
  onBlur,
  ...props
}: DowelPressableProps) {
  const [interaction, setInteraction] = useState(0)
  const setFlag = useCallback((flag: number, active: boolean) => {
    setInteraction((current: number) => (active ? current | flag : current & ~flag))
  }, [])

  return (
    <Pressable
      {...props}
      onHoverIn={(event: MouseEvent) => {
        setFlag(HOVERED, true)
        onHoverIn?.(event)
      }}
      onHoverOut={(event: MouseEvent) => {
        setFlag(HOVERED, false)
        onHoverOut?.(event)
      }}
      onFocus={(event: NativeSyntheticEvent<TargetedEvent>) => {
        setFlag(FOCUSED, true)
        onFocus?.(event)
      }}
      onBlur={(event: NativeSyntheticEvent<TargetedEvent>) => {
        setFlag(FOCUSED, false)
        onBlur?.(event)
      }}
      style={({ pressed }: PressableStateCallbackType) =>
        typeof style === 'function'
          ? style({
              pressed,
              hovered: (interaction & HOVERED) !== 0,
              focused: (interaction & FOCUSED) !== 0,
            })
          : style
      }
    />
  )
}
