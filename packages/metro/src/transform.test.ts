import assert from 'node:assert/strict'
import { test } from 'node:test'
import { transformHozoSource } from './transform.ts'

const LOGIN_SOURCE = `import { View, Text, Button } from '@hozo/core'

export function Login() {
  return (
    <View className="flex-1 items-center justify-center p-6">
      <Text className="text-xl font-bold">Welcome</Text>
      <Button className="mt-4 px-4 py-2">Continue</Button>
    </View>
  )
}
`

test('returns null for non-.tsx files', () => {
  assert.equal(transformHozoSource(LOGIN_SOURCE, 'Login.ts'), null)
})

test('returns null when there is no @hozo/core usage', () => {
  assert.equal(transformHozoSource('export const x = 1\n', 'x.tsx'), null)
})

test('strips the @hozo/core import and adds a react-native one', () => {
  const output = transformHozoSource(LOGIN_SOURCE, 'Login.tsx')
  assert.ok(output)
  assert.ok(!output!.includes("from '@hozo/core'"))
  assert.match(output!, /import \{[^}]*\} from 'react-native'/)
  // Button -> Pressable, so Pressable (not Button) is what should be
  // imported -- @hozo/core's Button has no RN equivalent, see hozo_native.
  assert.match(output!, /import \{[^}]*Pressable[^}]*\} from 'react-native'/)
  assert.ok(!output!.includes('Button,') && !output!.includes(', Button'))
})

test('maps the cross-platform PanResponder value to React Native', () => {
  const output = transformHozoSource(
    `
      import { View, PanResponder } from '@hozo/core'
      const pan = PanResponder.create({ onMoveShouldSetPanResponder: () => true })
      export function Drag() { return <View {...pan.panHandlers} /> }
    `,
    'Drag.tsx',
  )
  assert.ok(output)
  // No `StyleSheet`: this component has no classes, so nothing generated a
  // style object for it to create.
  assert.match(output, /import \{[^}]*View[^}]*PanResponder[^}]*\} from 'react-native'/)
  assert.ok(!output.includes('StyleSheet'), output)
  assert.match(output, /<View \{\.\.\.pan\.panHandlers\}/)
  assert.doesNotMatch(output, /@hozo\/core/)
})

test('injects a StyleSheet.create declaration and rewrites the JSX span', () => {
  const output = transformHozoSource(LOGIN_SOURCE, 'Login.tsx')
  assert.ok(output)
  assert.match(output!, /const hozoStyles = StyleSheet\.create\(\{/)
  assert.match(output!, /<View style=\{hozoStyles\.hozo_r0_0\}>/)
  assert.match(output!, /<Text style=\{hozoStyles\.hozo_r0_1\}>Welcome<\/Text>/)
  // The Pressable's label is wrapped: React Native crashes on a raw
  // string inside anything but a Text.
  assert.match(
    output!,
    /<Pressable style=\{hozoStyles\.hozo_r0_2\}[^>]*><Text>Continue<\/Text><\/Pressable>/,
  )
})

test('imports and directly lowers the canonical Image primitive', () => {
  const source = `import { Image } from '@hozo/core'
export function Cover() {
  return <Image className="w-20 h-20 object-cover" src="https://example.com/cover.jpg" alt="Cover" />
}
`
  const output = transformHozoSource(source, 'Cover.tsx')
  assert.ok(output)
  assert.match(output!, /import \{[^}]*Image[^}]*\} from 'react-native'/)
  assert.match(output!, /<Image style=\{hozoStyles\.hozo_r0_0\} accessibilityLabel=\{"Cover"\} source=\{\{ uri: "https:\/\/example\.com\/cover\.jpg" \}\} \/>/)
  assert.ok(!output!.includes("from '@hozo/core'"))
})

test('normalizes a platform-resolved Image source only when its type is dynamic', () => {
  const source = `import { Image } from '@hozo/core'
export function Logo() {
  return <Image src={logo} alt="Logo" onLoad={loaded} onError={failed} />
}
`
  const output = transformHozoSource(source, 'Logo.tsx')
  assert.ok(output)
  assert.match(output!, /import \{ hozoImageSource \} from '@hozo\/runtime'/)
  assert.match(output!, /source=\{hozoImageSource\(logo\)\}/)
  assert.match(output!, /onLoad=\{loaded\}/)
  assert.match(output!, /onError=\{failed\}/)
})

test('imports ScrollView without adding a runtime wrapper', () => {
  const source = `import { ScrollView, View } from '@hozo/core'
export function Rail() {
  return <ScrollView horizontal className="h-40"><View /></ScrollView>
}
`
  const output = transformHozoSource(source, 'Rail.tsx')
  assert.ok(output)
  assert.match(output!, /import \{[^}]*ScrollView[^}]*\} from 'react-native'/)
  assert.match(output!, /<ScrollView style=\{hozoStyles\.hozo_r0_0\} horizontal=\{true\}>/)
  assert.ok(!output!.includes('HozoScrollView'))
})

test('lowers Hozo primitives nested inside FlatList renderItem', () => {
  const source = `import { FlatList, Text } from '@hozo/core'
export function Rows() {
  return <FlatList className="h-40" data={rows} renderItem={({ item }) => <Text className="p-2">{item}</Text>} />
}
`
  const output = transformHozoSource(source, 'Rows.tsx')
  assert.ok(output)
  assert.match(output!, /import \{[^}]*FlatList[^}]*\} from 'react-native'/)
  assert.match(output!, /import \{[^}]*Text[^}]*\} from 'react-native'/)
  assert.match(output!, /renderItem=\{\(\{ item \}\) => <Text style=\{hozoStyles\.hozo_r0_1\}>\{item\}<\/Text>\}/)
  assert.ok(!output!.includes("from '@hozo/core'"))
})

test('fails the build on a Web-only utility instead of dropping it', () => {
  // `inline-block` has no React Native equivalent, so there is no correct output
  // to fall back to -- compiling anyway would look right on Web and be
  // silently wrong on device.
  const source = `import { View } from '@hozo/core'

export function Card() {
  return <View className="inline-block" />
}
`
  assert.throws(() => transformHozoSource(source, 'Card.tsx'), /WEB_ONLY_PROPERTY_ON_NATIVE/)
})

test('namespaces style/JSX identifiers per root so multiple components in one file do not collide', () => {
  const source = `import { View } from '@hozo/core'

export function First() {
  return <View className="p-4" />
}

export function Second() {
  return <View className="p-6" />
}
`
  const output = transformHozoSource(source, 'Multi.tsx')
  assert.ok(output)
  assert.match(output!, /hozo_r0_0/)
  assert.match(output!, /hozo_r1_0/)
  // The two components' style keys must actually differ, not just both
  // exist as separate unrelated strings.
  assert.notEqual(output!.match(/hozo_r0_0/)?.[0], undefined)
})

const DYNAMIC_SOURCE = `import { View } from '@hozo/core'

export function Card({ extra }) {
  return <View className={extra} />
}
`

test('hands an unreadable className to the generated resolver instead of failing', () => {
  // This used to be a build error: RN has no className to pass it through
  // to. It now resolves on device from the project-wide candidate map.
  const output = transformHozoSource(DYNAMIC_SOURCE, '/app/src/Card.tsx', '/app')
  assert.ok(output)
  assert.match(output!, /hozoClasses\(extra\)/)
  assert.match(output!, /import \{ hozoClasses \} from '\.\.\/node_modules\/\.hozo\/candidates\.native\.js'/)
})

test('does not import the candidate module into files that never call it', () => {
  // Otherwise every lowered file would depend on a module that only exists
  // once the config-time scan has run.
  const output = transformHozoSource(LOGIN_SOURCE, '/app/src/Login.tsx', '/app')
  assert.ok(output)
  assert.ok(!output!.includes('hozoClasses'))
})

test('says what is missing when the candidate module was never generated', () => {
  assert.throws(
    () => transformHozoSource(DYNAMIC_SOURCE, '/app/src/Card.tsx'),
    /generateCandidateModule/,
  )
})

test('splices hook declarations at the top of the component function', () => {
  // `dark:` and the breakpoints compile to a React hook. It has to be a
  // statement inside the component: a hook call inlined into the JSX
  // (`style={[a, useHozoDark() && b]}`) breaks the rules of hooks the
  // moment the element sits behind a conditional.
  const source = `import { View, Text } from '@hozo/core'

export function Card() {
  return (
    <View className="p-4 dark:bg-black md:flex-row">
      <Text className="dark:text-white">a</Text>
    </View>
  )
}
`
  const output = transformHozoSource(source, '/app/src/Card.tsx', '/app')
  assert.ok(output)
  assert.match(output!, /import \{[^}]*useHozoDark[^}]*\} from '@hozo\/runtime'/)
  assert.match(output!, /export function Card\(\) \{\n  const __hozoDark = useHozoDark\(\)/)
  assert.match(output!, /const __hozoBp_md = useHozoBreakpoint\('md'\)/)
  // One declaration, though two elements guard on it -- a second `const`
  // would redeclare the binding and change the hook order.
  assert.equal(output!.match(/const __hozoDark =/g)?.length, 1)
  assert.match(output!, /__hozoDark && hozoStyles\.hozo_r0_0_dark/)
})

test('refuses a hook where no statement can go', () => {
  const source = `import { View } from '@hozo/core'
const el = <View className="dark:bg-black" />
`
  assert.throws(
    () => transformHozoSource(source, '/app/src/x.tsx', '/app'),
    /need a React hook, which can only go inside a component function/,
  )
})

test('lowers ScrollView refresh through a native RefreshControl', () => {
  const source = `import { ScrollView, Text } from '@hozo/core'
export function Results({ refreshing, reload, horizontal }) {
  return <ScrollView className="h-40" horizontal={horizontal}
    refreshing={refreshing} onRefresh={reload}
    keyboardShouldPersistTaps="handled"
    showsHorizontalScrollIndicator={false}>
    <Text>row</Text>
  </ScrollView>
}
`
  const output = transformHozoSource(source, '/app/src/Results.tsx', '/app')
  assert.ok(output)
  assert.match(output!, /import \{[^}]*ScrollView[^}]*RefreshControl[^}]*StyleSheet[^}]*\} from 'react-native'/)
  assert.match(output!, /horizontal=\{horizontal\}/)
  assert.match(output!, /keyboardShouldPersistTaps=\{"handled"\}/)
  assert.match(output!, /showsHorizontalScrollIndicator=\{false\}/)
  assert.match(output!, /refreshControl=\{<RefreshControl refreshing=\{refreshing\} onRefresh=\{reload\} \/>\}/)
})

test('preserves Native FlatList virtualization controls while lowering nested components', () => {
  const source = `import { FlatList, Text } from '@hozo/core'
export function Results({ rows, loading, reload, loadMore }) {
  return <FlatList data={rows} numColumns={2}
    refreshing={loading} onRefresh={reload}
    onEndReached={loadMore} onEndReachedThreshold={0.5}
    showsVerticalScrollIndicator={false}
    ListEmptyComponent={<Text className="p-2">Empty</Text>}
    renderItem={({ item }) => <Text className="p-1">{item}</Text>} />
}
`
  const output = transformHozoSource(source, '/app/src/Results.tsx', '/app')
  assert.ok(output)
  assert.match(output!, /<FlatList accessibilityRole="list"/)
  assert.match(output!, /showsVerticalScrollIndicator=\{false\}/)
  assert.match(output!, /refreshing=\{loading\} onRefresh=\{reload\}/)
  assert.match(output!, /data=\{rows\} numColumns=\{2\}/)
  assert.match(output!, /onEndReached=\{loadMore\} onEndReachedThreshold=\{0\.5\}/)
  assert.match(output!, /ListEmptyComponent=\{<Text style=\{hozoStyles\.hozo_r0_1\}>Empty<\/Text>\}/)
  assert.match(output!, /renderItem=\{\(\{ item \}\) => <Text style=\{hozoStyles\.hozo_r0_2\}>\{item\}<\/Text>\}/)
})

test('compiles a plain React Native file, which is what an Expo app is', () => {
  // Proposal §2.1. The compiler always handled this; the gate in front of
  // it was `code.includes('@hozo/core')`, which skipped every Expo and
  // React Native project on the grounds that it had not been rewritten.
  const output = transformHozoSource(
    "import { View, Text } from 'react-native'\n" +
      'export function Card() { return (<View className="rounded-xl p-4"><Text className="font-bold">Hi</Text></View>) }\n',
    'Card.tsx',
  )
  assert.ok(output)
  assert.match(output, /StyleSheet\.create/)
  assert.match(output, /borderRadius: 12/)
  assert.match(output, /style=\{hozoStyles\.hozo_r0_0\}/)
  assert.ok(!output.includes('className='), 'the utility string should be gone')
})

test('adds only the react-native bindings the file does not already have', () => {
  // Re-declaring a name the file already imports is a SyntaxError, not a
  // duplicate: `Identifier 'View' has already been declared`. The
  // prepended import was unconditional while `@hozo/core` was the only
  // accepted source, because that import gets stripped and could not
  // collide.
  const output = transformHozoSource(
    "import { View, Text } from 'react-native'\n" +
      'export function Card() { return (<View className="p-4"><Text>Hi</Text></View>) }\n',
    'Card.tsx',
  )
  assert.ok(output)
  assert.equal(output.match(/from 'react-native'/g)?.length, 2, 'one original import plus one added')
  assert.match(output, /import \{ StyleSheet \} from 'react-native'/)
})

test('adds no import at all when the file already has everything', () => {
  const output = transformHozoSource(
    "import { StyleSheet, View } from 'react-native'\n" +
      'export function Card() { return (<View className="p-4" />) }\n',
    'Card.tsx',
  )
  assert.ok(output)
  assert.equal(output.match(/from 'react-native'/g)?.length, 1)
})

test('carries a foreign component and lowers the tree around it', () => {
  // The `@expo/ui` case, which is the reason resolution is per tag rather
  // than per file: that package exports `Text`, `Button`, `List`,
  // `ListItem`, `ScrollView` and `TextInput`, every one a native platform
  // component sharing nothing with the Hozo primitive but its spelling.
  const output = transformHozoSource(
    "import { View } from 'react-native'\nimport { Button, Host } from '@expo/ui/swift-ui'\n" +
      'export function Screen() { return (<View className="p-4"><Host><Button label="Save" /></Host></View>) }\n',
    'Screen.tsx',
  )
  assert.ok(output)
  assert.match(output, /style=\{hozoStyles\.hozo_r0_0\}/, 'the View should have lowered')
  assert.match(output, /<Host><Button label="Save" \/><\/Host>/, 'the @expo/ui half must survive')
  assert.ok(!output.includes('className='), 'the utility string should be gone')
})

test('a project can add its own module to the trusted list', () => {
  const source =
    "import { View } from './ui'\nexport function Card() { return (<View className=\"p-4\" />) }\n"
  assert.equal(transformHozoSource(source, 'Card.tsx'), null)
  const output = transformHozoSource(source, 'Card.tsx', undefined, undefined, [
    '@hozo/core',
    'react-native',
    './ui',
  ])
  assert.ok(output)
  assert.match(output, /StyleSheet\.create/)
})

test('does not import React Native’s component when the name is somebody else’s', () => {
  // `@expo/ui` exports `Text`, and the file already binds that name.
  // Adding React Native's beside it is `Identifier 'Text' has already been
  // declared` -- which is the same reason the compiler carried the tag
  // rather than lowering it.
  const output = transformHozoSource(
    "import { View } from 'react-native'\nimport { Text } from '@expo/ui/swift-ui'\n" +
      'export function S() { return (<View className="p-4"><Text>hi</Text></View>) }\n',
    'S.tsx',
  )
  assert.ok(output)
  assert.match(output, /import \{ StyleSheet \} from 'react-native'/)
  assert.ok(!/import \{[^}]*\bText\b[^}]*\} from 'react-native'/.test(output), output)
  assert.match(output, /<Text>hi<\/Text>/, 'the @expo/ui Text must survive')
})

test('leaves React Native’s own Button alone', () => {
  // The two share a name and no API: React Native's takes a `title` and
  // renders no children, Hozo's is a semantic primitive that lowers to a
  // Pressable wrapping its children. Trusting `react-native` wholesale
  // turned `<Button title="Go" onPress={f} />` into
  // `<Pressable onPress={f} title="Go"></Pressable>` -- a control that
  // renders nothing, in a file nobody asked Hozo to change.
  const source =
    "import { Button, View } from 'react-native'\n" +
    'export function S() { return <View><Button title="Go" onPress={f} /></View> }\n'
  assert.equal(transformHozoSource(source, 'S.tsx'), source, 'the file should come back untouched')
})

test('writes no StyleSheet for a file that produced no styles', () => {
  // Reached by every React Native file with no Tailwind classes, which in
  // a partly-migrated app is most of them. They were each getting a
  // `const hozoStyles = StyleSheet.create({})` for their trouble.
  const output = transformHozoSource(
    "import { FlatList } from 'react-native'\n" +
      'export function S() { return <FlatList data={[]} renderItem={() => null} /> }\n',
    'S.tsx',
  )
  assert.ok(output)
  assert.ok(!output.includes('StyleSheet'), output)
  // The semantic contribution still happens -- that is why the file is
  // rewritten at all.
  assert.match(output, /accessibilityRole="list"/)
})
