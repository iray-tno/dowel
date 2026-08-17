import { useCallback, useRef, useState } from 'react'
import {
  Animated,
  Easing,
  Pressable,
  StyleSheet,
  type MouseEvent,
  type NativeSyntheticEvent,
  type PressableProps,
  type PressableStateCallbackType,
  type StyleProp,
  type TargetedEvent,
  type ViewStyle,
} from 'react-native'

export interface DowelTransition {
  duration: number
  easing: 'linear' | 'ease-in' | 'ease-out' | 'ease-in-out'
}

export interface DowelPressableState extends PressableStateCallbackType {
  hovered: boolean
  focused: boolean
}

export interface DowelPressableProps extends Omit<PressableProps, 'style'> {
  style?: StyleProp<ViewStyle> | ((state: DowelPressableState) => StyleProp<ViewStyle>)
  dowelTransition?: DowelTransition
}

const HOVERED = 1
const FOCUSED = 2
const AnimatedPressable = Animated.createAnimatedComponent(Pressable)

function easingFor(name: DowelTransition['easing']) {
  switch (name) {
    case 'linear': return Easing.linear
    case 'ease-in': return Easing.in(Easing.ease)
    case 'ease-out': return Easing.out(Easing.ease)
    case 'ease-in-out': return Easing.inOut(Easing.ease)
  }
}

export function DowelPressable({
  style,
  onHoverIn,
  onHoverOut,
  onFocus,
  onBlur,
  dowelTransition,
  ...props
}: DowelPressableProps) {
  const [interaction, setInteraction] = useState(0)
  const interactionRef = useRef(0)
  const initialStyle = typeof style === 'function'
    ? style({ pressed: false, hovered: false, focused: false })
    : style
  const initialOpacity = StyleSheet.flatten(initialStyle)?.opacity
  const opacity = useRef(new Animated.Value(
    typeof initialOpacity === 'number' ? initialOpacity : 1,
  )).current
  const animateInteraction = useCallback((next: number) => {
    if (!dowelTransition || typeof style !== 'function') return
    const flattened = StyleSheet.flatten(style({
      pressed: false,
      hovered: (next & HOVERED) !== 0,
      focused: (next & FOCUSED) !== 0,
    }))
    const targetOpacity = typeof flattened?.opacity === 'number' ? flattened.opacity : 1
    Animated.timing(opacity, {
      toValue: targetOpacity,
      duration: dowelTransition.duration,
      easing: easingFor(dowelTransition.easing),
      useNativeDriver: true,
    }).start()
  }, [dowelTransition, opacity, style])
  const setFlag = useCallback((flag: number, active: boolean) => {
    const current = interactionRef.current
    const next = active ? current | flag : current & ~flag
    interactionRef.current = next
    animateInteraction(next)
    setInteraction(next)
  }, [animateInteraction])

  return (
    <AnimatedPressable
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
      style={({ pressed }: PressableStateCallbackType) => {
        const resolved = typeof style === 'function'
          ? style({
              pressed,
              hovered: (interaction & HOVERED) !== 0,
              focused: (interaction & FOCUSED) !== 0,
            })
          : style
        if (!dowelTransition) return resolved
        return [resolved, { opacity }]
      }}
    />
  )
}
