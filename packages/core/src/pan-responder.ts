import type {
  DowelResponderEvent,
  DowelTouchHistory,
  DowelTouchTrack,
  ResponderProps,
} from './responder.ts'

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

function centroidDimension(
  history: DowelTouchHistory,
  changedAfter: number,
  axis: 'x' | 'y',
  current: boolean,
) {
  const single = history.numberActiveTouches === 1
    ? history.touchBank[history.indexOfSingleActiveTouch]
    : undefined
  const tracks = single ? [single] : history.touchBank
  let total = 0
  let count = 0
  for (const track of tracks) {
    if (!track?.touchActive) continue
    const changed = single
      ? track.currentTimeStamp > changedAfter
      : track.currentTimeStamp >= changedAfter
    if (!changed) continue
    total += coordinate(track, axis, current)
    count++
  }
  return count > 0 ? total / count : -1
}

function coordinate(track: DowelTouchTrack, axis: 'x' | 'y', current: boolean) {
  if (current) return axis === 'x' ? track.currentPageX : track.currentPageY
  return axis === 'x' ? track.previousPageX : track.previousPageY
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
    const updateMove = (event: DowelResponderEvent) => {
      const history = event.touchHistory
      const timestamp = history.mostRecentTimeStamp
      if (gestureState._accountsForMovesUpTo === timestamp) return false
      const changedAfter = gestureState._accountsForMovesUpTo
      const x = centroidDimension(history, changedAfter, 'x', true)
      const y = centroidDimension(history, changedAfter, 'y', true)
      const previousX = centroidDimension(history, changedAfter, 'x', false)
      const previousY = centroidDimension(history, changedAfter, 'y', false)
      const deltaX = x - previousX
      const deltaY = y - previousY
      const elapsed = timestamp - changedAfter
      gestureState.numberActiveTouches = history.numberActiveTouches
      gestureState.moveX = x
      gestureState.moveY = y
      gestureState.dx += deltaX
      gestureState.dy += deltaY
      gestureState.vx = deltaX / elapsed
      gestureState.vy = deltaY / elapsed
      gestureState._accountsForMovesUpTo = timestamp
      return true
    }

    const panHandlers: ResponderProps = {
      onStartShouldSetResponder: (event) =>
        config.onStartShouldSetPanResponder?.(event, gestureState) ?? false,
      onMoveShouldSetResponder: (event) =>
        config.onMoveShouldSetPanResponder?.(event, gestureState) ?? false,
      onStartShouldSetResponderCapture: (event) => {
        if (event.nativeEvent.touches.length === 1) initialize(gestureState)
        gestureState.numberActiveTouches = event.touchHistory.numberActiveTouches
        return config.onStartShouldSetPanResponderCapture?.(event, gestureState) ?? false
      },
      onMoveShouldSetResponderCapture: (event) => {
        if (!updateMove(event)) return false
        return config.onMoveShouldSetPanResponderCapture?.(event, gestureState) ?? false
      },
      onResponderGrant: (event) => {
        gestureState.x0 = centroidDimension(event.touchHistory, 0, 'x', true)
        gestureState.y0 = centroidDimension(event.touchHistory, 0, 'y', true)
        gestureState.dx = 0
        gestureState.dy = 0
        config.onPanResponderGrant?.(event, gestureState)
        config.onShouldBlockNativeResponder?.(event, gestureState)
      },
      onResponderReject: (event) => config.onPanResponderReject?.(event, gestureState),
      onResponderStart: (event) => {
        gestureState.numberActiveTouches = event.touchHistory.numberActiveTouches
        config.onPanResponderStart?.(event, gestureState)
      },
      onResponderMove: (event) => {
        if (updateMove(event)) config.onPanResponderMove?.(event, gestureState)
      },
      onResponderEnd: (event) => {
        gestureState.numberActiveTouches = event.touchHistory.numberActiveTouches
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
