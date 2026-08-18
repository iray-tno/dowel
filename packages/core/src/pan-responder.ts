import type { DowelResponderEvent, DowelResponderTouch, ResponderProps } from './responder.ts'

export interface PanResponderGestureState {
  stateID: number
  moveX: number
  moveY: number
  x0: number
  y0: number
  dx: number
  dy: number
  vx: number
  vy: number
  numberActiveTouches: number
  _accountsForMovesUpTo: number
}

type ActiveCallback = (event: DowelResponderEvent, state: PanResponderGestureState) => boolean
type PassiveCallback = (event: DowelResponderEvent, state: PanResponderGestureState) => unknown

export interface PanResponderCallbacks {
  onMoveShouldSetPanResponder?: ActiveCallback
  onMoveShouldSetPanResponderCapture?: ActiveCallback
  onStartShouldSetPanResponder?: ActiveCallback
  onStartShouldSetPanResponderCapture?: ActiveCallback
  onPanResponderGrant?: PassiveCallback
  onPanResponderReject?: PassiveCallback
  onPanResponderStart?: PassiveCallback
  onPanResponderEnd?: PassiveCallback
  onPanResponderRelease?: PassiveCallback
  onPanResponderMove?: PassiveCallback
  onPanResponderTerminate?: PassiveCallback
  onPanResponderTerminationRequest?: ActiveCallback
  onShouldBlockNativeResponder?: ActiveCallback
}

export interface PanResponderInstance {
  panHandlers: ResponderProps
  getInteractionHandle(): number | null
}

let nextStateID = 1

function centroid(touches: DowelResponderTouch[], fallback: DowelResponderTouch) {
  const points = touches.length > 0 ? touches : [fallback]
  const total = points.reduce(
    (value, touch) => ({ x: value.x + touch.pageX, y: value.y + touch.pageY }),
    { x: 0, y: 0 },
  )
  return { x: total.x / points.length, y: total.y / points.length }
}

function initialize(state: PanResponderGestureState) {
  state.moveX = 0
  state.moveY = 0
  state.x0 = 0
  state.y0 = 0
  state.dx = 0
  state.dy = 0
  state.vx = 0
  state.vy = 0
  state.numberActiveTouches = 0
  state._accountsForMovesUpTo = 0
}

export const PanResponder = {
  create(config: PanResponderCallbacks): PanResponderInstance {
    const gestureState: PanResponderGestureState = {
      stateID: nextStateID++,
      moveX: 0,
      moveY: 0,
      x0: 0,
      y0: 0,
      dx: 0,
      dy: 0,
      vx: 0,
      vy: 0,
      numberActiveTouches: 0,
      _accountsForMovesUpTo: 0,
    }
    let previousX = 0
    let previousY = 0
    let previousTimestamp = 0

    const resetBaseline = (event: DowelResponderEvent) => {
      const point = centroid(event.nativeEvent.touches, event.nativeEvent)
      previousX = point.x
      previousY = point.y
      previousTimestamp = event.nativeEvent.timestamp
      gestureState.numberActiveTouches = event.nativeEvent.touches.length
    }

    const updateMove = (event: DowelResponderEvent) => {
      const timestamp = event.nativeEvent.timestamp
      if (gestureState._accountsForMovesUpTo === timestamp) return false
      const point = centroid(event.nativeEvent.touches, event.nativeEvent)
      const deltaX = point.x - previousX
      const deltaY = point.y - previousY
      const elapsed = timestamp - previousTimestamp
      gestureState.numberActiveTouches = event.nativeEvent.touches.length
      gestureState.moveX = point.x
      gestureState.moveY = point.y
      gestureState.dx += deltaX
      gestureState.dy += deltaY
      gestureState.vx = elapsed > 0 ? deltaX / elapsed : 0
      gestureState.vy = elapsed > 0 ? deltaY / elapsed : 0
      gestureState._accountsForMovesUpTo = timestamp
      previousX = point.x
      previousY = point.y
      previousTimestamp = timestamp
      return true
    }

    const panHandlers: ResponderProps = {
      onStartShouldSetResponder: (event) =>
        config.onStartShouldSetPanResponder?.(event, gestureState) ?? false,
      onMoveShouldSetResponder: (event) =>
        config.onMoveShouldSetPanResponder?.(event, gestureState) ?? false,
      onStartShouldSetResponderCapture: (event) => {
        if (event.nativeEvent.touches.length === 1) initialize(gestureState)
        gestureState.numberActiveTouches = event.nativeEvent.touches.length
        return config.onStartShouldSetPanResponderCapture?.(event, gestureState) ?? false
      },
      onMoveShouldSetResponderCapture: (event) => {
        if (!updateMove(event)) return false
        return config.onMoveShouldSetPanResponderCapture?.(event, gestureState) ?? false
      },
      onResponderGrant: (event) => {
        const point = centroid(event.nativeEvent.touches, event.nativeEvent)
        gestureState.x0 = point.x
        gestureState.y0 = point.y
        gestureState.dx = 0
        gestureState.dy = 0
        resetBaseline(event)
        config.onPanResponderGrant?.(event, gestureState)
        config.onShouldBlockNativeResponder?.(event, gestureState)
      },
      onResponderReject: (event) => config.onPanResponderReject?.(event, gestureState),
      onResponderStart: (event) => {
        resetBaseline(event)
        config.onPanResponderStart?.(event, gestureState)
      },
      onResponderMove: (event) => {
        if (updateMove(event)) config.onPanResponderMove?.(event, gestureState)
      },
      onResponderEnd: (event) => {
        resetBaseline(event)
        config.onPanResponderEnd?.(event, gestureState)
      },
      onResponderRelease: (event) => {
        config.onPanResponderRelease?.(event, gestureState)
        initialize(gestureState)
      },
      onResponderTerminate: (event) => {
        config.onPanResponderTerminate?.(event, gestureState)
        initialize(gestureState)
      },
      onResponderTerminationRequest: (event) =>
        config.onPanResponderTerminationRequest?.(event, gestureState) ?? true,
    }

    return { panHandlers, getInteractionHandle: () => null }
  },
}
