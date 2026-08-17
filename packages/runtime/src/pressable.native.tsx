import { useCallback, useMemo, useRef, useState } from 'react'
import {
  Animated,
  Easing,
  Pressable,
  StyleSheet,
  type GestureResponderEvent,
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
  opacity: boolean
  transform: boolean
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
const PRESSED = 4
const AnimatedPressable = Animated.createAnimatedComponent(Pressable)

function easingFor(name: DowelTransition['easing']) {
  switch (name) {
    case 'linear': return Easing.linear
    case 'ease-in': return Easing.in(Easing.ease)
    case 'ease-out': return Easing.out(Easing.ease)
    case 'ease-in-out': return Easing.inOut(Easing.ease)
  }
}

type TransformTarget = { key: string; value: number; degrees: boolean }
const transformRank = (key: string) => key.startsWith('translate') ? 0 : key.startsWith('rotate') ? 1 : 2

function transformTargets(style: StyleProp<ViewStyle>): TransformTarget[] {
  const merged = new Map<string, TransformTarget>()
  const visit = (part: any) => {
    if (!part) return
    if (Array.isArray(part)) return part.forEach(visit)
    const transform = StyleSheet.flatten(part)?.transform as any[] | undefined
    for (const entry of transform || []) {
      const [key, raw] = Object.entries(entry)[0] || []
      if (!key) continue
      if (key === 'scale' && typeof raw === 'number') {
        merged.set('scaleX', { key: 'scaleX', value: raw, degrees: false })
        merged.set('scaleY', { key: 'scaleY', value: raw, degrees: false })
      } else if (typeof raw === 'number') {
        merged.set(key, { key, value: raw, degrees: false })
      } else if (typeof raw === 'string' && raw.endsWith('deg')) {
        const value = Number(raw.slice(0, -3))
        if (Number.isFinite(value)) merged.set(key, { key, value, degrees: true })
      }
    }
  }
  visit(style)
  return [...merged.values()].sort((a, b) => transformRank(a.key) - transformRank(b.key))
}

function identityFor(target: TransformTarget) {
  return target.key.startsWith('scale') ? 1 : 0
}

export function DowelPressable({
  style,
  onHoverIn,
  onHoverOut,
  onFocus,
  onBlur,
  onPressIn,
  onPressOut,
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
  const transformSpecs = useMemo(() => {
    if (!dowelTransition?.transform || typeof style !== 'function') return []
    const states = [false, true].flatMap((pressed) =>
      [false, true].flatMap((hovered) =>
        [false, true].map((focused) => ({ pressed, hovered, focused })),
      ),
    )
    const merged = new Map<string, TransformTarget>()
    for (const state of states) {
      for (const target of transformTargets(style(state))) merged.set(target.key, target)
    }
    return [...merged.values()].sort((a, b) => transformRank(a.key) - transformRank(b.key))
  }, [dowelTransition?.transform, style])
  const transformValues = useRef(new Map<string, Animated.Value>()).current
  const initialTargets = new Map(transformTargets(initialStyle).map((target) => [target.key, target]))
  for (const spec of transformSpecs) {
    if (!transformValues.has(spec.key)) {
      transformValues.set(
        spec.key,
        new Animated.Value(initialTargets.get(spec.key)?.value ?? identityFor(spec)),
      )
    }
  }
  const animatedTransform = transformSpecs.map((spec) => ({
    [spec.key]: spec.degrees
      ? transformValues.get(spec.key)!.interpolate({
          inputRange: [-360, 360],
          outputRange: ['-360deg', '360deg'],
        })
      : transformValues.get(spec.key)!,
  }))
  const animateInteraction = useCallback((next: number) => {
    if (!dowelTransition || typeof style !== 'function') return
    const flattened = StyleSheet.flatten(style({
      pressed: (next & PRESSED) !== 0,
      hovered: (next & HOVERED) !== 0,
      focused: (next & FOCUSED) !== 0,
    }))
    const animations: Animated.CompositeAnimation[] = []
    if (dowelTransition.opacity) {
      const targetOpacity = typeof flattened?.opacity === 'number' ? flattened.opacity : 1
      animations.push(Animated.timing(opacity, {
        toValue: targetOpacity,
        duration: dowelTransition.duration,
        easing: easingFor(dowelTransition.easing),
        useNativeDriver: true,
      }))
    }
    if (dowelTransition.transform) {
      const targets = new Map(transformTargets(style({
        pressed: (next & PRESSED) !== 0,
        hovered: (next & HOVERED) !== 0,
        focused: (next & FOCUSED) !== 0,
      })).map((target) => [target.key, target.value]))
      for (const spec of transformSpecs) {
        animations.push(Animated.timing(transformValues.get(spec.key)!, {
          toValue: targets.get(spec.key) ?? identityFor(spec),
          duration: dowelTransition.duration,
          easing: easingFor(dowelTransition.easing),
          useNativeDriver: true,
        }))
      }
    }
    Animated.parallel(animations).start()
  }, [dowelTransition, opacity, style, transformSpecs, transformValues])
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
      onPressIn={(event: GestureResponderEvent) => {
        setFlag(PRESSED, true)
        onPressIn?.(event)
      }}
      onPressOut={(event: GestureResponderEvent) => {
        setFlag(PRESSED, false)
        onPressOut?.(event)
      }}
      style={({ pressed }: PressableStateCallbackType) => {
        const resolved = typeof style === 'function'
          ? style({
              pressed: pressed || (interaction & PRESSED) !== 0,
              hovered: (interaction & HOVERED) !== 0,
              focused: (interaction & FOCUSED) !== 0,
            })
          : style
        if (!dowelTransition) return resolved
        return [resolved, {
          ...(dowelTransition.opacity ? { opacity } : null),
          ...(dowelTransition.transform ? { transform: animatedTransform } : null),
        }]
      }}
    />
  )
}
