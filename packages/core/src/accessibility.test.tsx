// The fallbacks' accessibility props, rendered rather than read.
//
// This exists because of a bug that lived here unnoticed: ScrollView and
// FlatList wrote `aria-label`, `aria-description` and `aria-busy` and then
// spread `universalDomProps(...)` *after* them. That helper names all
// three unconditionally, and both components destructure
// `accessibilityLabel`/`accessibilityHint` out of the props it receives --
// so it named them `undefined`, and the later spread erased what the
// component had just set. A ScrollView given an `accessibilityLabel`
// rendered without one.
//
// Nothing caught it. There were no tests over these components at all, and
// the repository had never been type-checked; the first `tsc` run reported
// all six as "specified more than once, so this usage will be
// overwritten". So the test here is the general one -- every primitive
// that accepts an accessibility prop must put it in the markup -- rather
// than a narrow assertion about spread order, which is an implementation
// detail that could be got wrong again some other way.

import assert from 'node:assert/strict'
import test from 'node:test'
import { createElement, type ComponentType } from 'react'
import { renderToStaticMarkup } from 'react-dom/server'

import {
  Article,
  FlatList,
  Heading,
  List,
  ListItem,
  Nav,
  Paragraph,
  Pressable,
  ScrollView,
  Section,
  Text,
  View,
} from './index.tsx'

// Each primitive is handed the same bag of props, which is not a shape
// any one of their props types describes -- FlatList alone requires
// `data` and `renderItem`. `ComponentType<never>` is what lets them sit
// in one table; `render` below does the single cast that follows from it.
type Primitive = ComponentType<never>

/** Every primitive whose props extend `UniversalProps`. */
const PRIMITIVES: [string, Primitive][] = [
  ['View', View],
  ['Text', Text],
  ['Paragraph', Paragraph],
  ['Heading', Heading],
  ['Section', Section],
  ['Article', Article],
  ['Nav', Nav],
  ['List', List],
  ['ListItem', ListItem],
  ['ScrollView', ScrollView],
  ['FlatList', FlatList],
  // Pressable was not in this list when the file was written, and writing
  // it is how that turned up: `PressableProps extends ResponderProps`
  // alone, so `testID`, `accessibilityState` and the rest were not part
  // of its contract at all. React Native's own Pressable takes every one
  // of them, and an interactive element is exactly where `aria-checked`,
  // `aria-expanded` and `aria-selected` earn their keep.
  //
  // The `onLayout` question that made it look like a design decision
  // rather than an oversight had already been answered by `View`, which
  // has the same pair: `useLayoutRef` returns the ref, and the responder
  // takes that same ref. There is only ever one.
  ['Pressable', Pressable],
]

function render(component: Primitive, props: Record<string, unknown>) {
  const renderable = component as ComponentType<Record<string, unknown>>
  // `data`/`renderItem` keep FlatList renderable; an empty list never
  // calls the latter, and every assertion here is about the container.
  return renderToStaticMarkup(createElement(renderable, { data: [], renderItem: () => null, ...props }))
}

test('every primitive renders the accessibilityLabel it was given', () => {
  for (const [name, component] of PRIMITIVES) {
    const html = render(component, { accessibilityLabel: 'Message list' })
    assert.match(html, /aria-label="Message list"/, `${name} dropped its accessibilityLabel`)
  }
})

test('every primitive renders the accessibilityHint it was given', () => {
  for (const [name, component] of PRIMITIVES) {
    const html = render(component, { accessibilityHint: 'Scrolls to newest' })
    assert.match(
      html,
      /aria-description="Scrolls to newest"/,
      `${name} dropped its accessibilityHint`,
    )
  }
})

test('accessibilityState reaches the markup through the universal props', () => {
  for (const [name, component] of PRIMITIVES) {
    const html = render(component, {
      accessibilityState: { disabled: true, expanded: false },
    })
    assert.match(html, /aria-disabled="true"/, `${name} dropped accessibilityState.disabled`)
    assert.match(html, /aria-expanded="false"/, `${name} dropped accessibilityState.expanded`)
  }
})

// `refreshing` is the scrolling containers' own prop rather than a
// universal one, and it was the third casualty of the same spread: it
// compiles to `aria-busy`, which `universalDomProps` also names.
test('a refreshing scroll container is busy', () => {
  for (const [name, component] of [
    ['ScrollView', ScrollView],
    ['FlatList', FlatList],
  ] as [string, Primitive][]) {
    const html = render(component, { refreshing: true })
    assert.match(html, /aria-busy="true"/, `${name} did not report itself busy`)
  }
})

// The other half of the ordering rule: a component's explicit attribute
// must win, but only where it has one. Nothing else `universalDomProps`
// carries may be lost by moving the spread first.
test('testID and nativeID survive alongside the explicit attributes', () => {
  for (const [name, component] of PRIMITIVES) {
    const html = render(component, {
      testID: 'inbox',
      nativeID: 'inbox-root',
      accessibilityLabel: 'Inbox',
    })
    assert.match(html, /data-testid="inbox"/, `${name} dropped testID`)
    assert.match(html, /id="inbox-root"/, `${name} dropped nativeID`)
    assert.match(html, /aria-label="Inbox"/, `${name} dropped accessibilityLabel`)
  }
})

// Pressable folds both spellings of the disabled state, the way React
// Native does and the way the compiled path does. Two sources for one
// attribute is how they end up disagreeing, which is the shape of every
// bug this file exists over.
test('a Pressable disabled through accessibilityState is inoperable, not just announced', () => {
  const enabled = render(Pressable, { accessibilityRole: 'button', onPress: () => {} })
  assert.match(enabled, /tabindex="0"/, 'an enabled Pressable is not in the tab order')

  const html = render(Pressable, {
    accessibilityRole: 'button',
    accessibilityState: { disabled: true },
    onPress: () => {},
  })
  assert.match(html, /aria-disabled="true"/, 'the state was not announced')
  // Out of the tab order, reachable by focus(). docs/decisions/001 rule 1a.
  assert.match(html, /tabindex="-1"/, 'the state did not remove the tab stop')
  // And the styling hook, so `disabled:` reaches it -- `:disabled` never
  // could, this being a <div>.
  assert.match(html, /data-hozo-disabled=""/, 'the styling hook is missing')
})

// The contract the generated CSS depends on. `disabled:` and its family
// compile to `[data-hozo-…]` selectors, which work on any element --
// `:disabled` matches form controls only, and most of these are a `<div>`.
// So a primitive rendering through this file has to carry the attribute or
// the styles come off in the fallback path while working in the compiled
// one, which is the least visible way for the two to disagree.
test('every primitive carries the styling hook the generated CSS matches on', () => {
  for (const [name, component] of PRIMITIVES) {
    const html = render(component, { accessibilityState: { disabled: true } })
    assert.match(html, /data-hozo-disabled=""/, `${name} dropped the disabled styling hook`)
    // Presence, not a value: React renders `data-x={false}` as the string
    // "false", which an attribute selector matches.
    const enabled = render(component, { accessibilityState: { disabled: false } })
    assert.doesNotMatch(enabled, /data-hozo-disabled/, `${name} marks an enabled element disabled`)
  }
})
