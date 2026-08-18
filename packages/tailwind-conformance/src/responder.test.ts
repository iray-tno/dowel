import assert from 'node:assert/strict'
import { test } from 'node:test'
import type { PointerEvent as ReactPointerEvent } from 'react'

import {
  createResponderDomProps,
  type DowelResponderEvent,
  type ResponderProps,
} from '../../core/src/responder.ts'

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
  let moveEvent: DowelResponderEvent | undefined
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
  secondHandlers.onPointerCancel?.(pointer(second, 2))

  assert.deepEqual(lifecycle, [
    'first grant',
    'first terminate',
    'second grant',
    'second terminate',
  ])
})
