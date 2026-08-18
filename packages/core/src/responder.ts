import { useEffect, useRef, type PointerEvent as ReactPointerEvent, type PointerEventHandler, type RefObject } from 'react'

export interface DowelResponderTouch {
  identifier: number
  locationX: number
  locationY: number
  pageX: number
  pageY: number
  target: EventTarget | null
  timestamp: number
}

export interface DowelResponderEvent {
  nativeEvent: DowelResponderTouch & {
    changedTouches: DowelResponderTouch[]
    touches: DowelResponderTouch[]
  }
  touchHistory: DowelTouchHistory
  preventDefault(): void
  stopPropagation(): void
}

export interface DowelTouchTrack {
  touchActive: boolean
  currentPageX: number
  currentPageY: number
  currentTimeStamp: number
  previousPageX: number
  previousPageY: number
  previousTimeStamp: number
}

export interface DowelTouchHistory {
  touchBank: DowelTouchTrack[]
  numberActiveTouches: number
  indexOfSingleActiveTouch: number
  mostRecentTimeStamp: number
}

export interface ResponderProps {
  onStartShouldSetResponder?: (event: DowelResponderEvent) => boolean
  onStartShouldSetResponderCapture?: (event: DowelResponderEvent) => boolean
  onMoveShouldSetResponder?: (event: DowelResponderEvent) => boolean
  onMoveShouldSetResponderCapture?: (event: DowelResponderEvent) => boolean
  onResponderGrant?: (event: DowelResponderEvent) => void
  onResponderStart?: (event: DowelResponderEvent) => void
  onResponderMove?: (event: DowelResponderEvent) => void
  onResponderEnd?: (event: DowelResponderEvent) => void
  onResponderRelease?: (event: DowelResponderEvent) => void
  onResponderReject?: (event: DowelResponderEvent) => void
  onResponderTerminate?: (event: DowelResponderEvent) => void
  onResponderTerminationRequest?: (event: DowelResponderEvent) => boolean
}

interface Registration {
  element: HTMLElement
  pointerIds: Set<number>
  props: RefObject<ResponderProps>
}

let activeResponder: Registration | undefined

interface PointerSnapshot {
  identifier: number
  clientX: number
  clientY: number
  pageX: number
  pageY: number
  target: EventTarget | null
  timestamp: number
}

const activePointers = new Map<number, PointerSnapshot>()
const pointerHistory = new Map<number, DowelTouchTrack>()
const trackedEvents = new WeakSet<object>()
let globalCleanupInstalled = false

function endPointer(
  pointerId: number,
  pageX?: number,
  pageY?: number,
  timestamp?: number,
) {
  activePointers.delete(pointerId)
  const track = pointerHistory.get(pointerId)
  if (track) {
    track.touchActive = false
    if (pageX !== undefined && pageY !== undefined && timestamp !== undefined) {
      track.previousPageX = track.currentPageX
      track.previousPageY = track.currentPageY
      track.previousTimeStamp = track.currentTimeStamp
      track.currentPageX = pageX
      track.currentPageY = pageY
      track.currentTimeStamp = timestamp
    }
  }
}

function ensureGlobalPointerCleanup() {
  if (globalCleanupInstalled || typeof window === 'undefined') return
  globalCleanupInstalled = true
  const remove = (event: PointerEvent) => {
    endPointer(event.pointerId, event.pageX, event.pageY, event.timeStamp)
  }
  window.addEventListener('pointerup', remove)
  window.addEventListener('pointercancel', remove)
}

function eventKey(event: ReactPointerEvent<HTMLElement>): object {
  return typeof event.nativeEvent === 'object' && event.nativeEvent !== null
    ? event.nativeEvent
    : event
}

function trackPointer(
  event: ReactPointerEvent<HTMLElement>,
  phase: 'start' | 'move' | 'end',
): boolean {
  const key = eventKey(event)
  if (trackedEvents.has(key)) return false
  trackedEvents.add(key)
  if (phase === 'end') {
    endPointer(event.pointerId, event.pageX, event.pageY, event.timeStamp)
  } else {
    ensureGlobalPointerCleanup()
    if (phase === 'start' && activePointers.size === 0) pointerHistory.clear()
    activePointers.set(event.pointerId, {
      identifier: event.pointerId,
      clientX: event.clientX,
      clientY: event.clientY,
      pageX: event.pageX,
      pageY: event.pageY,
      target: event.target,
      timestamp: event.timeStamp,
    })
    const previous = pointerHistory.get(event.pointerId)
    pointerHistory.set(event.pointerId, {
      touchActive: true,
      currentPageX: event.pageX,
      currentPageY: event.pageY,
      currentTimeStamp: event.timeStamp,
      previousPageX: phase === 'move' && previous ? previous.currentPageX : event.pageX,
      previousPageY: phase === 'move' && previous ? previous.currentPageY : event.pageY,
      previousTimeStamp: phase === 'move' && previous
        ? previous.currentTimeStamp
        : event.timeStamp,
    })
  }
  return true
}

function releaseRegistration(props: RefObject<ResponderProps>) {
  const incumbent = activeResponder
  if (!incumbent || incumbent.props !== props) return
  activeResponder = undefined
  for (const pointerId of incumbent.pointerIds) {
    if (incumbent.element.hasPointerCapture?.(pointerId)) {
      incumbent.element.releasePointerCapture?.(pointerId)
    }
  }
}

function responderEvent(event: ReactPointerEvent<HTMLElement>, ended = false): DowelResponderEvent {
  const rect = event.currentTarget.getBoundingClientRect()
  const toTouch = (pointer: PointerSnapshot): DowelResponderTouch => ({
    identifier: pointer.identifier,
    locationX: pointer.clientX - rect.left,
    locationY: pointer.clientY - rect.top,
    pageX: pointer.pageX,
    pageY: pointer.pageY,
    target: pointer.target,
    timestamp: pointer.timestamp,
  })
  const changed = toTouch({
    identifier: event.pointerId,
    clientX: event.clientX,
    clientY: event.clientY,
    pageX: event.pageX,
    pageY: event.pageY,
    target: event.target,
    timestamp: event.timeStamp,
  })
  const touches = [...activePointers.values()].map(toTouch)
  const touchBank = [...pointerHistory.values()]
  const activeIndices = touchBank
    .map((track, index) => track.touchActive ? index : -1)
    .filter((index) => index >= 0)
  return {
    nativeEvent: {
      ...changed,
      changedTouches: [changed],
      touches: ended ? touches.filter((touch) => touch.identifier !== event.pointerId) : touches,
    },
    touchHistory: {
      touchBank,
      numberActiveTouches: activePointers.size,
      indexOfSingleActiveTouch: activeIndices.length === 1 ? activeIndices[0] : -1,
      mostRecentTimeStamp: touchBank.reduce(
        (latest, track) => Math.max(latest, track.currentTimeStamp),
        0,
      ),
    },
    preventDefault: () => event.preventDefault(),
    stopPropagation: () => event.stopPropagation(),
  }
}

function claim(
  element: HTMLElement,
  props: RefObject<ResponderProps>,
  event: ReactPointerEvent<HTMLElement>,
): boolean {
  if (!event.isPrimary || activeResponder?.element === element) return false

  const value = responderEvent(event)
  if (activeResponder) {
    const incumbent = activeResponder
    const allowsTermination = incumbent.props.current.onResponderTerminationRequest?.(value) ?? true
    if (!allowsTermination) {
      props.current.onResponderReject?.(value)
      return false
    }
    activeResponder = undefined
    incumbent.props.current.onResponderTerminate?.(value)
    for (const pointerId of incumbent.pointerIds) {
      if (incumbent.element.hasPointerCapture?.(pointerId)) {
        incumbent.element.releasePointerCapture?.(pointerId)
      }
    }
  }

  activeResponder = { element, pointerIds: new Set([event.pointerId]), props }
  element.setPointerCapture?.(event.pointerId)
  props.current.onResponderGrant?.(value)
  props.current.onResponderStart?.(value)
  return true
}

function finish(element: HTMLElement, event: ReactPointerEvent<HTMLElement>, terminated: boolean) {
  const incumbent = activeResponder
  if (!incumbent || incumbent.element !== element || !incumbent.pointerIds.has(event.pointerId)) return
  incumbent.pointerIds.delete(event.pointerId)
  const value = responderEvent(event, true)
  incumbent.props.current.onResponderEnd?.(value)
  if (element.hasPointerCapture?.(event.pointerId)) element.releasePointerCapture?.(event.pointerId)
  if (!terminated && incumbent.pointerIds.size > 0) return
  activeResponder = undefined
  if (terminated) incumbent.props.current.onResponderTerminate?.(value)
  else incumbent.props.current.onResponderRelease?.(value)
}

export function useResponderDomProps<T extends HTMLElement>(
  elementRef: RefObject<T | null>,
  props: ResponderProps,
  enabled = true,
) {
  const propsRef = useRef(props)
  propsRef.current = props
  useEffect(() => () => releaseRegistration(propsRef), [enabled])
  return createResponderDomProps(elementRef, propsRef, enabled)
}

export function createResponderDomProps<T extends HTMLElement>(
  elementRef: RefObject<T | null>,
  propsRef: RefObject<ResponderProps>,
  enabled = true,
) {
  if (!enabled) return {}

  const negotiate = (
    shouldSet: ((event: DowelResponderEvent) => boolean) | undefined,
    event: ReactPointerEvent<T>,
  ) => {
    const element = elementRef.current
    if (!element || activeResponder?.element === element || !shouldSet?.(responderEvent(event))) return false
    return claim(element, propsRef, event)
  }

  const onPointerDown: PointerEventHandler<T> = (event) => {
    const isNewPointer = trackPointer(event, 'start')
    const element = elementRef.current
    const incumbent = activeResponder
    if (isNewPointer && element && incumbent?.element === element) {
      incumbent.pointerIds.add(event.pointerId)
      element.setPointerCapture?.(event.pointerId)
      propsRef.current.onResponderStart?.(responderEvent(event))
      return
    }
    // The responder negotiation bubbles deepest-first. Once a child wins,
    // ancestors must not make a second claim from the same pointer start.
    if (negotiate(propsRef.current.onStartShouldSetResponder, event)) event.stopPropagation()
  }
  const onPointerDownCapture: PointerEventHandler<T> = (event) => {
    const isNewPointer = trackPointer(event, 'start')
    const element = elementRef.current
    const incumbent = activeResponder
    if (isNewPointer && element && incumbent?.element === element) {
      incumbent.pointerIds.add(event.pointerId)
      element.setPointerCapture?.(event.pointerId)
      propsRef.current.onResponderStart?.(responderEvent(event))
      event.stopPropagation()
      return
    }
    if (negotiate(propsRef.current.onStartShouldSetResponderCapture, event)) event.stopPropagation()
  }
  const onPointerMove: PointerEventHandler<T> = (event) => {
    trackPointer(event, 'move')
    const element = elementRef.current
    const incumbent = activeResponder
    if (element && incumbent && incumbent.element === element && incumbent.pointerIds.has(event.pointerId)) {
      propsRef.current.onResponderMove?.(responderEvent(event))
    } else {
      if (negotiate(propsRef.current.onMoveShouldSetResponder, event)) event.stopPropagation()
    }
  }
  const onPointerMoveCapture: PointerEventHandler<T> = (event) => {
    trackPointer(event, 'move')
    if (negotiate(propsRef.current.onMoveShouldSetResponderCapture, event)) event.stopPropagation()
  }
  const onPointerUp: PointerEventHandler<T> = (event) => {
    trackPointer(event, 'end')
    finish(event.currentTarget, event, false)
  }
  const onPointerCancel: PointerEventHandler<T> = (event) => {
    trackPointer(event, 'end')
    finish(event.currentTarget, event, true)
  }
  const onLostPointerCapture: PointerEventHandler<T> = (event) => {
    trackPointer(event, 'end')
    finish(event.currentTarget, event, true)
  }

  return {
    onPointerDown,
    onPointerDownCapture,
    onPointerMove,
    onPointerMoveCapture,
    onPointerUp,
    onPointerCancel,
    onLostPointerCapture,
  }
}
