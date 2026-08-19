import assert from 'node:assert/strict'
import { test } from 'node:test'

import { lowerModule } from './lower.ts'
import { DEFAULT_PRIMITIVE_SOURCES, decideSources, foreignSourceMessage } from './sources.ts'

const rn = "import { View, Text } from 'react-native'\n"
const card = "export function Card() { return (<View className=\"p-4\"><Text>Hi</Text></View>) }\n"

test('a plain React Native file is compilable', () => {
  // Proposal §2.1: existing source is the input, with no migration to a
  // Hozo-specific API. True of the compiler since it was written, and
  // false of every integration until the gate stopped being a
  // `code.includes('@hozo/core')` substring test.
  const decision = decideSources(rn + card, DEFAULT_PRIMITIVE_SOURCES)
  assert.ok(decision.compilable)
  assert.deepEqual(decision.foreign, [])
})

test('a primitive from an unknown module is not compilable', () => {
  // The hazard the tag-based compiler cannot see on its own: someone
  // else's `View` has its own props and its own layout, and lowering it to
  // a `<div>` because the tag is spelled `View` replaces their component
  // with something else.
  const decision = decideSources("import { View } from 'some-ui-kit'\n" + card, DEFAULT_PRIMITIVE_SOURCES)
  assert.ok(!decision.compilable)
  assert.deepEqual(decision.foreign, [{ local: 'View', module: 'some-ui-kit' }])
})

test('one unrecognised primitive disqualifies the whole file', () => {
  // Not per-tag, because the compiler lowers by tag name and cannot be
  // told to lower this `View` and not that one.
  const mixed = rn + "import { Pressable } from 'some-ui-kit'\n" + card
  assert.ok(!decideSources(mixed, DEFAULT_PRIMITIVE_SOURCES).compilable)
})

test('a project can name its own module', () => {
  // The re-export case: a design system wrapping the primitives it
  // re-exports is still handing Hozo the components it knows.
  const source = "import { View } from './ui'\n" + card
  assert.ok(!decideSources(source, DEFAULT_PRIMITIVE_SOURCES).compilable)
  assert.ok(decideSources(source, [...DEFAULT_PRIMITIVE_SOURCES, './ui']).compilable)
})

test('the message names the module, since the tag is the same either way', () => {
  const message = foreignSourceMessage(
    'Card.tsx',
    [{ local: 'View', module: 'some-ui-kit' }],
    DEFAULT_PRIMITIVE_SOURCES,
  )
  assert.match(message, /some-ui-kit/)
  assert.match(message, /react-native/)
  assert.match(message, /sources/)
})

test('lowering a React Native file produces Web output', () => {
  const lowered = lowerModule(rn + card, 'Card.tsx', 'Card.tsx', undefined)
  assert.ok(lowered?.lowered)
  assert.match(lowered.code, /<div/)
  assert.match(lowered.css, /padding-top: 16px/)
})

test('a file of somebody else’s components is not Hozo’s business', () => {
  // No diagnostic, and nothing read past the first check. A project whose
  // own components happen to be named `View` is not doing anything wrong,
  // and a warning on every one of its files would be noise about a
  // decision Hozo was never asked to make.
  const source = "import { View } from 'some-ui-kit'\n" + card
  assert.equal(lowerModule(source, 'Card.tsx', 'Card.tsx', undefined), undefined)
})

test('a mixed file is declined, untouched, with a reason', () => {
  // Here the warning earns its place: the file imports from a module Hozo
  // does handle, so its author has every reason to expect it lowered, and
  // the reason it wasn't is a single import they can see.
  const source = rn + "import { Pressable } from 'some-ui-kit'\n" + card
  const lowered = lowerModule(source, 'Card.tsx', 'Card.tsx', undefined)
  assert.ok(lowered)
  assert.equal(lowered.lowered, false)
  assert.equal(lowered.code, source, 'a declined file must not be rewritten')
  assert.equal(lowered.diagnostics[0].code, 'PRIMITIVE_FROM_UNKNOWN_MODULE')
  // No stylesheet was written, so the caller must not import one.
  assert.equal(lowered.cssFileName, '')
})

test('a file with no primitives at all is skipped outright', () => {
  assert.equal(lowerModule('export const x = 1\n', 'a.tsx', 'a.tsx', undefined), undefined)
})
