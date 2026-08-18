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
  preventDefault(): void
  stopPropagation(): void
}

export interface ResponderProps {
  onStartShouldSetResponder?: (event: DowelResponderEvent) => boolean
  onStartShouldSetResponderCapture?: (event: DowelResponderEvent) => boolean
  onMoveShouldSetResponder?: (event: DowelResponderEvent) => boolean
  onMoveShouldSetResponderCapture?: (event: DowelResponderEvent) => boolean
  onResponderGrant?: (event: DowelResponderEvent) => void
  onResponderMove?: (event: DowelResponderEvent) => void
  onResponderRelease?: (event: DowelResponderEvent) => void
  onResponderReject?: (event: DowelResponderEvent) => void
  onResponderTerminate?: (event: DowelResponderEvent) => void
  onResponderTerminationRequest?: (event: DowelResponderEvent) => boolean
}

interface Registration {
  element: HTMLElement
  pointerId: number
  props: RefObject<ResponderProps>
}

let activeResponder: Registration | undefined

function releaseRegistration(props: RefObject<ResponderProps>) {
  const incumbent = activeResponder
  if (!incumbent || incumbent.props !== props) return
  activeResponder = undefined
  if (incumbent.element.hasPointerCapture?.(incumbent.pointerId)) {
    incumbent.element.releasePointerCapture?.(incumbent.pointerId)
  }
}

function responderEvent(event: ReactPointerEvent<HTMLElement>, ended = false): DowelResponderEvent {
  const rect = event.currentTarget.getBoundingClientRect()
  const touch: DowelResponderTouch = {
    identifier: event.pointerId,
    locationX: event.clientX - rect.left,
    locationY: event.clientY - rect.top,
    pageX: event.pageX,
    pageY: event.pageY,
    target: event.target,
    timestamp: event.timeStamp,
  }
  return {
    nativeEvent: { ...touch, changedTouches: [touch], touches: ended ? [] : [touch] },
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
    if (incumbent.element.hasPointerCapture?.(incumbent.pointerId)) {
      incumbent.element.releasePointerCapture?.(incumbent.pointerId)
    }
  }

  activeResponder = { element, pointerId: event.pointerId, props }
  element.setPointerCapture?.(event.pointerId)
  props.current.onResponderGrant?.(value)
  return true
}

function finish(element: HTMLElement, event: ReactPointerEvent<HTMLElement>, terminated: boolean) {
  const incumbent = activeResponder
  if (!incumbent || incumbent.element !== element || incumbent.pointerId !== event.pointerId) return
  activeResponder = undefined
  const value = responderEvent(event, true)
  if (terminated) incumbent.props.current.onResponderTerminate?.(value)
  else incumbent.props.current.onResponderRelease?.(value)
  if (element.hasPointerCapture?.(event.pointerId)) element.releasePointerCapture?.(event.pointerId)
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
    // The responder negotiation bubbles deepest-first. Once a child wins,
    // ancestors must not make a second claim from the same pointer start.
    if (negotiate(propsRef.current.onStartShouldSetResponder, event)) event.stopPropagation()
  }
  const onPointerDownCapture: PointerEventHandler<T> = (event) => {
    if (negotiate(propsRef.current.onStartShouldSetResponderCapture, event)) event.stopPropagation()
  }
  const onPointerMove: PointerEventHandler<T> = (event) => {
    const element = elementRef.current
    const incumbent = activeResponder
    if (element && incumbent && incumbent.element === element && incumbent.pointerId === event.pointerId) {
      propsRef.current.onResponderMove?.(responderEvent(event))
    } else {
      if (negotiate(propsRef.current.onMoveShouldSetResponder, event)) event.stopPropagation()
    }
  }
  const onPointerMoveCapture: PointerEventHandler<T> = (event) => {
    if (negotiate(propsRef.current.onMoveShouldSetResponderCapture, event)) event.stopPropagation()
  }
  const onPointerUp: PointerEventHandler<T> = (event) => finish(event.currentTarget, event, false)
  const onPointerCancel: PointerEventHandler<T> = (event) => finish(event.currentTarget, event, true)
  const onLostPointerCapture: PointerEventHandler<T> = (event) => finish(event.currentTarget, event, true)

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
