import assert from 'node:assert/strict'
import { test } from 'node:test'

import { renderNative, renderNativeWithLayouts, type Tree } from './native-render.ts'

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

test('semantic document primitives retain native accessibility intent', () => {
  const tree = renderNative(
    `
    import { Section, Heading, Paragraph } from '@dowel/core'
    export function Article() {
      return <Section><Heading level={3}>Title</Heading><Paragraph>Body</Paragraph></Section>
    }
    `,
    'Article',
  )
  assert.equal(tree?.type, 'View')
  const [heading, paragraph] = children(tree)
  assert.equal(heading.type, 'Text')
  assert.equal(heading.props.accessibilityRole, 'header')
  assert.equal(paragraph.type, 'Text')
})

test('article and navigation landmarks retain Native roles', () => {
  const tree = renderNative(
    `
    import { Article, Nav } from '@dowel/core'
    export function Shell() {
      return <Article><Nav accessibilityLabel="Primary" /></Article>
    }
    `,
    'Shell',
  )
  assert.equal(tree?.type, 'View')
  assert.equal(tree?.props.role, 'article')
  const [nav] = children(tree)
  assert.equal(nav.type, 'View')
  assert.equal(nav.props.role, 'navigation')
  assert.equal(nav.props.accessibilityLabel, 'Primary')
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
  assert.equal(rows.some((row) => typeof row.props.onLayout === 'function'), false)
})

test('DowelGrid measured rows settle after layout and respond to width changes', () => {
  const source = `
    import { View, Text } from '@dowel/core'
    export function Grid() {
      return (
        <View className="grid grid-cols-2 gap-2">
          <Text className="row-span-2">Tall</Text><Text>Top</Text><Text>Bottom</Text>
        </View>
      )
    }
  `
  // Target order is the measured container followed by its three absolute
  // cells. The second pass is what a real renderer reports after row sizes
  // have been imposed; the third represents a parent width/rotation change.
  const tree = renderNativeWithLayouts(source, 'Grid', [
    [
      { width: 300, height: 0 },
      { width: 146, height: 70 },
      { width: 146, height: 20 },
      { width: 146, height: 30 },
    ],
    [
      { width: 300, height: 70 },
      { width: 146, height: 70 },
      { width: 146, height: 26 },
      { width: 146, height: 36 },
    ],
    [
      { width: 400, height: 70 },
      { width: 196, height: 70 },
      { width: 196, height: 26 },
      { width: 196, height: 36 },
    ],
  ])

  // The authored View remains the semantic/styling container; the solver is
  // its only child and owns the absolute placement layer.
  assert.deepEqual(tree?.props.style, { gap: 8 })
  const [grid] = children(tree)
  assert.deepEqual(grid.props.style, { position: 'relative', alignSelf: 'stretch', height: 70 })
  const cells = children(grid)
  assert.deepEqual(cells.map((cell) => cell.props.style), [
    { position: 'absolute', left: 0, top: 0, width: 196, height: 70 },
    { position: 'absolute', left: 204, top: 0, width: 196, height: 26 },
    { position: 'absolute', left: 204, top: 34, width: 196, height: 36 },
  ])
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

test('Link renders as a native link interaction with its destination', () => {
  const tree = renderNative(
    `
    import { Link } from '@dowel/core'
    export function Docs() {
      return <Link href="https://example.com" accessibilityLabel="Documentation">Docs</Link>
    }
    `,
    'Docs',
  )
  assert.equal(tree?.type, 'Pressable')
  assert.equal(tree?.props.accessibilityRole, 'link')
  assert.equal(tree?.props.accessibilityLabel, 'Documentation')
  assert.equal(typeof tree?.props.onPress, 'function')
})

test('Image maps its URI and alternative onto React Native Image props', () => {
  const tree = renderNative(
    `
    import { Image } from '@dowel/core'
    export function Cover() {
      return <Image className="w-20 h-20 object-cover" src="https://example.com/cover.jpg" alt="Cover" />
    }
    `,
    'Cover',
  )
  assert.equal(tree?.type, 'Image')
  assert.deepEqual(tree?.props.source, { uri: 'https://example.com/cover.jpg' })
  assert.equal(tree?.props.accessibilityLabel, 'Cover')
  assert.deepEqual(tree?.props.style, { width: 80, height: 80, objectFit: 'cover' })
})

test('Image retains a Metro local asset and load state callbacks', () => {
  const loaded = () => {}
  const failed = () => {}
  const tree = renderNative(
    `
    import { Image } from '@dowel/core'
    export function Logo() {
      return <Image src={logo} alt="Logo" onLoad={loaded} onError={failed} />
    }
    `,
    'Logo',
    { logo: 42, loaded, failed },
  )
  assert.equal(tree?.props.source, 42)
  assert.equal(tree?.props.onLoad, loaded)
  assert.equal(tree?.props.onError, failed)
})

test('ScrollView directly uses the Native viewport and keeps child layout explicit', () => {
  const tree = renderNative(
    `
    import { ScrollView, View, Text } from '@dowel/core'
    export function Rail() {
      return (
        <ScrollView horizontal className="h-40">
          <View className="flex-row gap-4"><Text>One</Text><Text>Two</Text></View>
        </ScrollView>
      )
    }
    `,
    'Rail',
  )
  assert.equal(tree?.type, 'ScrollView')
  assert.equal(tree?.props.horizontal, true)
  assert.deepEqual(tree?.props.style, { height: 160 })
  const content = tree?.children?.[0]
  assert.equal(typeof content === 'string' ? content : content?.type, 'View')
  if (typeof content !== 'string' && content) {
    assert.deepEqual(content.props.style, { flexDirection: 'row', gap: 16 })
  }
})

test('FlatList stays virtualized and its renderItem body is compiled', () => {
  const tree = renderNative(
    `
    import { FlatList, Text } from '@dowel/core'
    export function Rows() {
      return <FlatList className="h-40" data={rows} renderItem={({ item }) => <Text className="p-2">{item}</Text>} />
    }
    `,
    'Rows',
    { rows: ['One', 'Two'] },
  )
  assert.equal(tree?.type, 'FlatList')
  assert.equal(tree?.props.accessibilityRole, 'list')
  assert.deepEqual(tree?.props.data, ['One', 'Two'])
  const renderItem = tree?.props.renderItem as ((info: { item: string }) => { type: string; props: Record<string, unknown> }) | undefined
  assert.equal(typeof renderItem, 'function')
  const item = renderItem!({ item: 'One' })
  assert.equal(item.type, 'Text')
  assert.deepEqual(item.props.style, { paddingTop: 8, paddingRight: 8, paddingBottom: 8, paddingLeft: 8 })
})

test('focus-visible installs modality events only on an interaction that asks for them', () => {
  const tree = renderNative(
    `
    import { Pressable } from '@dowel/core'
    export function Save() {
      return <Pressable className="focus-visible:opacity-50" accessibilityRole="button">Save</Pressable>
    }
    `,
    'Save',
  )
  assert.equal(tree?.type, 'Pressable')
  assert.equal(typeof tree?.props.onFocus, 'function')
  assert.equal(typeof tree?.props.onPointerDown, 'function')
  assert.equal(typeof tree?.props.onKeyDown, 'function')
  assert.equal(typeof tree?.props.style, 'function')
})
