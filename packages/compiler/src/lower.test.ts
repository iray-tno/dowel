import assert from 'node:assert/strict'
import { test } from 'node:test'

import { lowerModule, namespaceHozoClasses, referencesHozoPrimitive } from './lower.ts'

const file = 'Page.tsx'

test('lowers a component and namespaces its classes', () => {
  const source =
    `import { View } from '@hozo/core'\n` +
    `export function Page() { return <View className="p-4">x</View> }\n`
  const lowered = lowerModule(source, file, file, undefined)

  assert.ok(lowered)
  assert.match(lowered.code, /<div/)
  assert.match(lowered.code, /hozo-r0-0/)
  assert.match(lowered.css, /\.hozo-r0-0/)
  assert.equal(lowered.cssFileName, 'Page.tsx.hozo.css')
})

test('leaves alone what it has nothing to do with', () => {
  assert.equal(lowerModule('export const x = 1\n', file, file, undefined), undefined)
  assert.equal(lowerModule("import { View } from '@hozo/core'\n", 'a.ts', 'a.ts', undefined), undefined)
})

test('a derived module gets its own companion stylesheet', () => {
  // Route-splitting frameworks transform several query-qualified modules
  // from one source file, and each owns different JSX -- one shared path
  // would let the last transform overwrite the others' CSS.
  const source =
    `import { View } from '@hozo/core'\n` +
    `export function Page() { return <View className="p-4">x</View> }\n`
  const plain = lowerModule(source, file, file, undefined)!
  const derived = lowerModule(source, `${file}?ssr=true`, file, undefined)!
  assert.notEqual(plain.cssFileName, derived.cssFileName)
  assert.match(derived.cssFileName, /^Page\.tsx\..+\.hozo\.css$/)
})

test('keeps the @hozo/core import when a primitive survived lowering', () => {
  // Regression, and it broke at *runtime* rather than at build: a stray
  // template literal turned `\b` into a backspace character, so the word
  // boundary matched nothing, `referencesHozoPrimitive` answered "no" for
  // every input, and the import was stripped out from under a
  // `PanResponder` the compiler had deliberately carried through.
  assert.ok(referencesHozoPrimitive('const pan = PanResponder.create({})'))
  assert.ok(referencesHozoPrimitive('const Label = Text\n'))
  assert.ok(!referencesHozoPrimitive('const x = 1\n'))
  // A word match, so a longer identifier that merely contains one is not
  // a reference.
  assert.ok(!referencesHozoPrimitive('const ViewModel = 1\n'))

  const source =
    `import { PanResponder, View } from '@hozo/core'\n` +
    `const pan = PanResponder.create({})\n` +
    `export function Page() { return <View className="p-4">x</View> }\n`
  const lowered = lowerModule(source, file, file, undefined)!
  assert.ok(lowered.code.includes('@hozo/core'), 'the import a survivor needs was stripped')
})

test('the import survives even when lowering left nothing using it', () => {
  // Not an oversight: the import statement is part of the text
  // `referencesHozoPrimitive` searches, so it always finds one. What
  // removes the module from the bundle is the bundler's own
  // unused-specifier elision, which cannot be wrong the way a regex can.
  // Asserted so that a future attempt to make the strip "work" has to
  // face what it would break.
  const source =
    `import { View } from '@hozo/core'\n` +
    `export function Page() { return <View className="p-4">x</View> }\n`
  const lowered = lowerModule(source, file, file, undefined)!
  assert.ok(lowered.code.includes('@hozo/core'))
  assert.ok(!/<View/.test(lowered.code), 'the tag itself should be gone')
})

test('namespacing leaves the shared base class alone', () => {
  assert.equal(namespaceHozoClasses('hozo-view hozo-0', 2), 'hozo-view hozo-r2-0')
})
