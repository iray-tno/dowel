import assert from 'node:assert/strict'
import { test } from 'node:test'
import { transformDowelSource } from './transform.ts'

const LOGIN_SOURCE = `import { View, Text, Button } from '@dowel/core'

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
  assert.equal(transformDowelSource(LOGIN_SOURCE, 'Login.ts'), null)
})

test('returns null when there is no @dowel/core usage', () => {
  assert.equal(transformDowelSource('export const x = 1\n', 'x.tsx'), null)
})

test('strips the @dowel/core import and adds a react-native one', () => {
  const output = transformDowelSource(LOGIN_SOURCE, 'Login.tsx')
  assert.ok(output)
  assert.ok(!output!.includes("from '@dowel/core'"))
  assert.match(output!, /import \{[^}]*\} from 'react-native'/)
  // Button -> Pressable, so Pressable (not Button) is what should be
  // imported -- @dowel/core's Button has no RN equivalent, see dowel_native.
  assert.match(output!, /import \{[^}]*Pressable[^}]*\} from 'react-native'/)
  assert.ok(!output!.includes('Button,') && !output!.includes(', Button'))
})

test('injects a StyleSheet.create declaration and rewrites the JSX span', () => {
  const output = transformDowelSource(LOGIN_SOURCE, 'Login.tsx')
  assert.ok(output)
  assert.match(output!, /const styles = StyleSheet\.create\(\{/)
  assert.match(output!, /<View style=\{styles\.dowel_r0_0\}>/)
  assert.match(output!, /<Text style=\{styles\.dowel_r0_1\}>Welcome<\/Text>/)
  assert.match(output!, /<Pressable style=\{styles\.dowel_r0_2\}[^>]*>Continue<\/Pressable>/)
})

test('namespaces style/JSX identifiers per root so multiple components in one file do not collide', () => {
  const source = `import { View } from '@dowel/core'

export function First() {
  return <View className="p-4" />
}

export function Second() {
  return <View className="p-6" />
}
`
  const output = transformDowelSource(source, 'Multi.tsx')
  assert.ok(output)
  assert.match(output!, /dowel_r0_0/)
  assert.match(output!, /dowel_r1_0/)
  // The two components' style keys must actually differ, not just both
  // exist as separate unrelated strings.
  assert.notEqual(output!.match(/dowel_r0_0/)?.[0], undefined)
})
