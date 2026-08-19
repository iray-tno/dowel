import assert from 'node:assert/strict'
import { test } from 'node:test'

import { lowerModule } from './lower.ts'
import { DEFAULT_PRIMITIVE_SOURCES } from './sources.ts'

const rn = "import { View, Text } from 'react-native'\n"
const card = 'export function Card() { return (<View className="p-4"><Text>Hi</Text></View>) }\n'

test('a plain React Native file compiles', () => {
  // Proposal §2.1: existing source is the input, with no migration to a
  // Hozo-specific API. True of the compiler since it was written, and
  // false of every integration until the gate stopped being a
  // `code.includes('@hozo/core')` substring test.
  const lowered = lowerModule(rn + card, 'Card.tsx', 'Card.tsx', undefined)
  assert.ok(lowered?.lowered)
  assert.match(lowered.code, /<div/)
  assert.match(lowered.css, /padding-top: 16px/)
})

test("a file of somebody else's components is left alone", () => {
  // No diagnostic and nothing parsed. A project whose own components
  // happen to be named `View` is not doing anything wrong.
  const source = "import { View } from 'some-ui-kit'\n" + card
  assert.equal(lowerModule(source, 'Card.tsx', 'Card.tsx', undefined), undefined)
})

test('a mixed Expo file lowers one half and carries the other', () => {
  // The case the whole `sources` mechanism exists for. `@expo/ui` exports
  // `Text`, `Button`, `List`, `ListItem`, `ScrollView` and `TextInput` --
  // every one a native platform component sharing nothing with the Hozo
  // primitive but its spelling. Refusing the file would leave the
  // `react-native` half uncompiled; accepting it would turn a SwiftUI
  // button into a `<div>`.
  const source =
    "import { View } from 'react-native'\n" +
    "import { Button, Host } from '@expo/ui/swift-ui'\n" +
    'export function Screen() {\n' +
    '  return (<View className="p-4"><Host><Button label="Save" /></Host></View>)\n' +
    '}\n'
  const lowered = lowerModule(source, 'Screen.tsx', 'Screen.tsx', undefined)

  assert.ok(lowered?.lowered)
  assert.match(lowered.code, /<div className="hozo-view hozo-r0-0">/, 'the View should have lowered')
  assert.match(lowered.code, /<Host><Button label="Save" \/><\/Host>/, 'the @expo/ui half must survive untouched')
  assert.match(lowered.css, /padding-top: 16px/)
})

test('the same name resolves differently in the same file', () => {
  // `Text` from `react-native` lowers; `Text` from `@expo/ui` is carried.
  // A tag-name-only compiler cannot tell these apart, which is why the
  // module list travels into it.
  const source =
    "import { View, Text } from 'react-native'\n" +
    "import { Text as NativeText } from '@expo/ui/swift-ui'\n" +
    'export function Screen() {\n' +
    '  return (<View><Text className="font-bold">a</Text><NativeText>b</NativeText></View>)\n' +
    '}\n'
  const lowered = lowerModule(source, 'Screen.tsx', 'Screen.tsx', undefined)
  assert.ok(lowered?.lowered)
  assert.match(lowered.code, /<span className="hozo-r0-1">a<\/span>/)
  assert.match(lowered.code, /<NativeText>b<\/NativeText>/)
})

test('a project can add its own module to the trusted list', () => {
  // The re-export case: a design system wrapping the primitives it
  // re-exports is still handing Hozo the components it knows.
  const source = "import { View } from './ui'\n" + card
  assert.equal(lowerModule(source, 'Card.tsx', 'Card.tsx', undefined), undefined)

  const lowered = lowerModule(source, 'Card.tsx', 'Card.tsx', undefined, {
    sources: [...DEFAULT_PRIMITIVE_SOURCES, './ui'],
  })
  assert.ok(lowered?.lowered)
  assert.match(lowered.code, /<div/)
})

test('a file with no primitives at all is skipped outright', () => {
  assert.equal(lowerModule('export const x = 1\n', 'a.tsx', 'a.tsx', undefined), undefined)
})
