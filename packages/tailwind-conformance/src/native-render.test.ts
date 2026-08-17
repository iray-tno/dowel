import assert from 'node:assert/strict'
import { test } from 'node:test'

import { renderNative, type Tree } from './native-render.ts'

function children(tree: Tree): Tree[] {
  return ((tree?.children ?? []) as (Tree | string)[]).filter(
    (child): child is Tree => typeof child === 'object' && child !== null,
  )
}

test('a compiled component builds the tree it was meant to', () => {
  // The Native counterpart of rendering the Web output: the generated
  // module is assembled by the real Metro transformer, evaluated, and run.
  // Nothing established any of that before -- the type check says each
  // style is one React Native would accept, and said nothing about whether
  // the module runs.
  const tree = renderNative(
    `
    import { View, Text } from '@dowel/core'
    export function Card() {
      return <View className="p-4"><Text className="text-xl">Hi</Text></View>
    }
    `,
    'Card',
  )
  assert.equal(tree?.type, 'View')
  assert.deepEqual(tree?.props.style, {
    paddingTop: 16,
    paddingRight: 16,
    paddingBottom: 16,
    paddingLeft: 16,
  })
  const [text] = children(tree)
  assert.equal(text.type, 'Text')
  assert.deepEqual(text.props.style, { fontSize: 20, lineHeight: 28 })
})

test('DowelSpaced puts the spacing on every child but the last', () => {
  // The component's first execution. Its rule -- `:not(:last-child)`, and
  // the parent's style behind the child's own so the child wins -- was
  // tested as a pure function; this is the React half of it.
  const tree = renderNative(
    `
    import { View, Text } from '@dowel/core'
    export function List() {
      return (
        <View className="space-y-4">
          <Text className="text-xl">One</Text>
          <Text>Two</Text>
          <Text>Three</Text>
        </View>
      )
    }
    `,
    'List',
  )
  const items = children(tree)
  assert.equal(items.length, 3)

  const spacing = { marginTop: 0, marginBottom: 16 }
  // Spacing first, the element's own second: React Native resolves a style
  // array last-wins, so this is what lets a child's own margin override.
  assert.deepEqual(items[0].props.style, [spacing, { fontSize: 20, lineHeight: 28 }])
  // The second child has no style of its own, so the slot is empty.
  assert.deepEqual(items[1].props.style, [spacing, undefined])
  // The last child is `:last-child` on Web and gets nothing here either.
  assert.equal(items[2].props.style, undefined)
})

test('DowelGrid auto-places unequal tracks without a measurement pass', () => {
  const tree = renderNative(
    `
    import { View, Text } from '@dowel/core'
    export function Grid() {
      return (
        <View className="grid grid-cols-[120px_2fr_1fr] gap-4">
          <Text className="col-start-2 col-span-2">Wide</Text><Text>Two</Text><Text>Three</Text>
        </View>
      )
    }
    `,
    'Grid',
  )
  const rows = children(tree)
  assert.equal(rows.length, 2)
  assert.deepEqual(rows[0].props.style, { flexDirection: 'row', columnGap: 16 })
  const firstRow = children(rows[0])
  assert.deepEqual(firstRow.map((cell) => cell.props.style), [
    { flexBasis: 120, flexGrow: 0, flexShrink: 0 },
    { flexBasis: 16, flexGrow: 3, flexShrink: 1 },
  ])
  assert.equal(children(firstRow[0]).length, 0)
  assert.equal(children(rows[1]).length, 3)
  assert.equal(children(children(rows[1])[2]).length, 0)
})

test('a text style set on a View reaches the Text underneath it', () => {
  // React Native inherits text styles only from a Text, so the compiler
  // carries them down. Checked here on the rendered tree rather than on the
  // emitted string, which is where it would look right either way.
  const tree = renderNative(
    `
    import { View, Text } from '@dowel/core'
    export function Card() {
      return <View className="text-xl text-red-500"><Text>Hi</Text></View>
    }
    `,
    'Card',
  )
  // Nothing left on the View: it has no `fontSize` to hold.
  assert.equal(tree?.props.style, undefined)
  const [text] = children(tree)
  assert.deepEqual(text.props.style, { fontSize: 20, lineHeight: 28, color: '#fb2c36' })
})

test('a Dialog renders its children only while it is open', () => {
  const source = `
    import { Dialog, Text } from '@dowel/core'
    export function Confirm() {
      return (
        <Dialog className="p-6" open={showing} onClose={dismiss} accessibilityLabel="Confirm">
          <Text>Delete?</Text>
        </Dialog>
      )
    }
  `
  const open = renderNative(source, 'Confirm', { showing: true, dismiss: () => {} })
  assert.equal(open?.type, 'Modal')
  assert.equal(open?.props.visible, true)
  // The accessible name and the modal semantics reach the view inside,
  // which is what a screen reader reads when the dialog appears.
  const [inner] = children(open)
  assert.equal(inner.props.accessibilityLabel, 'Confirm')
  assert.equal(inner.props.accessibilityViewIsModal, true)
  assert.deepEqual(inner.props.style, {
    paddingTop: 24,
    paddingRight: 24,
    paddingBottom: 24,
    paddingLeft: 24,
  })

  const closed = renderNative(source, 'Confirm', { showing: false, dismiss: () => {} })
  assert.equal(closed?.props.visible, false)
})

test('truncation reaches the prop React Native carries it on', () => {
  const tree = renderNative(
    `
    import { Text } from '@dowel/core'
    export function Clamped() {
      return <Text className="line-clamp-2">a long line</Text>
    }
    `,
    'Clamped',
  )
  assert.equal(tree?.props.numberOfLines, 2)
})
