import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { PointerEvent as ReactPointerEvent } from 'react'

import {
  createResponderDomProps,
  type HozoResponderEvent,
  type ResponderProps,
} from '../../core/src/responder.ts'
import { PanResponder, type PanResponderGestureState } from '../../core/src/pan-responder.ts'

interface FakeElement {
  captured: Set<number>
  getBoundingClientRect(): { left: number; top: number }
  setPointerCapture(pointerId: number): void
  hasPointerCapture(pointerId: number): boolean
  releasePointerCapture(pointerId: number): void
}

function element(): FakeElement {
  const captured = new Set<number>()
  return {
    captured,
    getBoundingClientRect: () => ({ left: 10, top: 20 }),
    setPointerCapture: (id) => captured.add(id),
    hasPointerCapture: (id) => captured.has(id),
    releasePointerCapture: (id) => captured.delete(id),
  }
}

function pointer(currentTarget: FakeElement, pointerId: number, overrides = {}) {
  return {
    currentTarget,
    target: currentTarget,
    pointerId,
    isPrimary: true,
    clientX: 17,
    clientY: 29,
    pageX: 117,
    pageY: 129,
    timeStamp: 42,
    preventDefault() {},
    stopPropagation() {},
    ...overrides,
  } as unknown as ReactPointerEvent<HTMLElement>
}

test('the Web responder bridge grants, moves, releases, and normalizes coordinates', () => {
  const target = element()
  const lifecycle: string[] = []
  let moveEvent: HozoResponderEvent | undefined
  const props: ResponderProps = {
    onStartShouldSetResponder: () => true,
    onResponderGrant: () => lifecycle.push('grant'),
    onResponderMove: (event) => {
      lifecycle.push('move')
      moveEvent = event
    },
    onResponderRelease: (event) => {
      lifecycle.push('release')
      assert.deepEqual(event.nativeEvent.touches, [])
    },
  }
  const handlers = createResponderDomProps(
    { current: target as unknown as HTMLElement },
    { current: props },
  )

  let propagationStopped = false
  handlers.onPointerDown?.(pointer(target, 7, { stopPropagation: () => { propagationStopped = true } }))
  assert.equal(propagationStopped, true)
  assert.deepEqual([...target.captured], [7])
  handlers.onPointerMove?.(pointer(target, 7))
  assert.equal(moveEvent?.nativeEvent.identifier, 7)
  assert.equal(moveEvent?.nativeEvent.locationX, 7)
  assert.equal(moveEvent?.nativeEvent.locationY, 9)
  assert.equal(moveEvent?.nativeEvent.pageX, 117)
  handlers.onPointerUp?.(pointer(target, 7))

  assert.deepEqual(lifecycle, ['grant', 'move', 'release'])
  assert.deepEqual([...target.captured], [])
})

test('an incumbent responder can reject a competing responder', () => {
  const first = element()
  const second = element()
  const lifecycle: string[] = []
  const firstHandlers = createResponderDomProps(
    { current: first as unknown as HTMLElement },
    { current: {
      onStartShouldSetResponder: () => true,
      onResponderGrant: () => lifecycle.push('first grant'),
      onResponderTerminationRequest: () => false,
      onResponderRelease: () => lifecycle.push('first release'),
    } },
  )
  const secondHandlers = createResponderDomProps(
    { current: second as unknown as HTMLElement },
    { current: {
      onStartShouldSetResponder: () => true,
      onResponderGrant: () => lifecycle.push('second grant'),
      onResponderReject: () => lifecycle.push('second reject'),
    } },
  )

  firstHandlers.onPointerDown?.(pointer(first, 1))
  secondHandlers.onPointerDown?.(pointer(second, 2))
  firstHandlers.onPointerUp?.(pointer(first, 1))
  secondHandlers.onPointerUp?.(pointer(second, 2))

  assert.deepEqual(lifecycle, ['first grant', 'second reject', 'first release'])
})

test('an accepted transfer terminates the incumbent and pointer cancellation terminates the winner', () => {
  const first = element()
  const second = element()
  const lifecycle: string[] = []
  const firstHandlers = createResponderDomProps(
    { current: first as unknown as HTMLElement },
    { current: {
      onStartShouldSetResponder: () => true,
      onResponderGrant: () => lifecycle.push('first grant'),
      onResponderTerminate: () => lifecycle.push('first terminate'),
    } },
  )
  const secondHandlers = createResponderDomProps(
    { current: second as unknown as HTMLElement },
    { current: {
      onStartShouldSetResponder: () => true,
      onResponderGrant: () => lifecycle.push('second grant'),
      onResponderTerminate: () => lifecycle.push('second terminate'),
    } },
  )

  firstHandlers.onPointerDown?.(pointer(first, 1))
  secondHandlers.onPointerDown?.(pointer(second, 2))
  firstHandlers.onPointerUp?.(pointer(first, 1))
  secondHandlers.onPointerCancel?.(pointer(second, 2))

  assert.deepEqual(lifecycle, [
    'first grant',
    'first terminate',
    'second grant',
    'second terminate',
  ])
})

test('multiple pointers stay in one responder until the last pointer ends', () => {
  const target = element()
  const lifecycle: string[] = []
  const touchCounts: number[] = []
  const handlers = createResponderDomProps(
    { current: target as unknown as HTMLElement },
    { current: {
      onStartShouldSetResponder: () => true,
      onResponderStart: (event) => touchCounts.push(event.nativeEvent.touches.length),
      onResponderEnd: (event) => touchCounts.push(event.nativeEvent.touches.length),
      onResponderMove: (event) => {
        assert.deepEqual(event.nativeEvent.touches.map((touch) => touch.identifier), [11, 12])
        lifecycle.push('move')
      },
      onResponderRelease: () => lifecycle.push('release'),
    } },
  )

  handlers.onPointerDown?.(pointer(target, 11))
  handlers.onPointerDown?.(pointer(target, 12, { isPrimary: false }))
  handlers.onPointerMove?.(pointer(target, 12, { isPrimary: false, pageX: 140 }))
  handlers.onPointerUp?.(pointer(target, 11))
  assert.deepEqual(lifecycle, ['move'])
  handlers.onPointerUp?.(pointer(target, 12, { isPrimary: false }))

  assert.deepEqual(touchCounts, [1, 2, 1, 0])
  assert.deepEqual(lifecycle, ['move', 'release'])
})

test('PanResponder.create derives displacement, velocity, and active touch count', () => {
  const target = element()
  const moves: PanResponderGestureState[] = []
  const releases: PanResponderGestureState[] = []
  const pan = PanResponder.create({
    onStartShouldSetPanResponder: () => true,
    onPanResponderMove: (_event, state) => moves.push({ ...state }),
    onPanResponderRelease: (_event, state) => releases.push({ ...state }),
  })
  const handlers = createResponderDomProps(
    { current: target as unknown as HTMLElement },
    { current: pan.panHandlers },
  )

  handlers.onPointerDown?.(pointer(target, 21, { timeStamp: 10 }))
  handlers.onPointerMove?.(pointer(target, 21, {
    clientX: 27,
    clientY: 49,
    pageX: 127,
    pageY: 149,
    timeStamp: 20,
  }))
  handlers.onPointerUp?.(pointer(target, 21, { timeStamp: 30 }))

  assert.equal(moves.length, 1)
  assert.equal(moves[0].x0, 117)
  assert.equal(moves[0].y0, 129)
  assert.equal(moves[0].moveX, 127)
  assert.equal(moves[0].moveY, 149)
  assert.equal(moves[0].dx, 10)
  assert.equal(moves[0].dy, 20)
  assert.equal(moves[0].vx, 0.5)
  assert.equal(moves[0].vy, 1)
  assert.equal(moves[0].numberActiveTouches, 1)
  assert.equal(releases.length, 1)
  assert.equal(releases[0].numberActiveTouches, 0)
  assert.equal(pan.getInteractionHandle(), null)
})

test('PanResponder accumulates the RN touch-history cluster when pointers move alternately', () => {
  const target = element()
  const moves: PanResponderGestureState[] = []
  const pan = PanResponder.create({
    onStartShouldSetPanResponder: () => true,
    onPanResponderMove: (_event, state) => moves.push({ ...state }),
  })
  const handlers = createResponderDomProps(
    { current: target as unknown as HTMLElement },
    { current: pan.panHandlers },
  )

  handlers.onPointerDown?.(pointer(target, 31, { pageX: 100, timeStamp: 10 }))
  handlers.onPointerDown?.(pointer(target, 32, {
    isPrimary: false,
    pageX: 200,
    timeStamp: 11,
  }))
  // Account for both newly active pointers before measuring alternating moves.
  handlers.onPointerMove?.(pointer(target, 32, {
    isPrimary: false,
    pageX: 200,
    timeStamp: 12,
  }))
  handlers.onPointerMove?.(pointer(target, 31, { pageX: 110, timeStamp: 22 }))
  handlers.onPointerMove?.(pointer(target, 32, {
    isPrimary: false,
    pageX: 210,
    timeStamp: 32,
  }))

  assert.equal(moves.at(-1)?.moveX, 160)
  // RN includes tracks exactly on the previous accounting boundary (`>=`),
  // so the first 10px move contributes 5px and is completed on the next move.
  assert.equal(moves.at(-1)?.dx, 15)
  assert.equal(moves.at(-1)?.vx, 1)
  assert.equal(moves.at(-1)?.numberActiveTouches, 2)
})
