// Does the Web backend's output survive the consumer's own type-check?
//
// It lands in the user's `.tsx`, so their `tsc` sees it. `next build`
// type-checks by default. Until this file existed nothing asked, and the
// answer was no.

import assert from 'node:assert/strict'
import { test } from 'node:test'

import { compile } from '@hozo/compiler'

import { typeCheckWeb } from './typecheck-web.ts'

/** Compiles a source and type-checks every component it produced. */
function check(source: string, freeNames: string[] = []) {
  const compiled = compile(source)
  const components = compiled.map((component, index) => ({
    name: `C${index}`,
    jsx: component.jsx,
  }))
  // Whatever the compiler said this module needs, imported for real.
  const runtimeImports = compiled.flatMap((component) => component.runtimeImports)
  return typeCheckWeb(components, freeNames, runtimeImports)
}

function assertClean(source: string, freeNames: string[] = []) {
  const errors = check(source, freeNames)
  assert.deepEqual(
    errors.map((error) => error.line),
    [],
  )
}

test('an interactive Pressable type-checks', () => {
  // The case that was broken: `tabIndex` is a `number` in React's types,
  // and this emitted `tabIndex="0"`.
  assertClean(
    `
    import { Pressable, Text } from '@hozo/core'
    export function Save() {
      return <Pressable accessibilityRole="button" onPress={save}><Text>Save</Text></Pressable>
    }
    `,
    ['save'],
  )
})

test('the semantic primitives type-check', () => {
  assertClean(`
    import { Section, Heading, Paragraph, Article, Nav, List, ListItem } from '@hozo/core'
    export function Page() {
      return (
        <Article>
          <Nav accessibilityLabel="Primary" />
          <Section className="p-4">
            <Heading level={2}>Title</Heading>
            <Paragraph>Body</Paragraph>
            <List><ListItem>One</ListItem></List>
          </Section>
        </Article>
      )
    }
  `)
})

test('a disabled Button type-checks', () => {
  assertClean(
    `
    import { Button } from '@hozo/core'
    export function Save() {
      return <Button disabled={busy} onPress={save}>Save</Button>
    }
    `,
    ['busy', 'save'],
  )
})

test('accessibility state and value type-check', () => {
  assertClean(
    `
    import { Pressable, Text } from '@hozo/core'
    export function Toggle() {
      return (
        <Pressable
          accessibilityRole="button"
          accessibilityState={{ expanded: open }}
          accessibilityLabel="Details"
          onPress={toggle}
        ><Text>Details</Text></Pressable>
      )
    }
    `,
    ['open', 'toggle'],
  )
})

test('a scroll container and a text input type-check', () => {
  assertClean(
    `
    import { ScrollView, TextInput, Text } from '@hozo/core'
    export function Form() {
      return (
        <ScrollView className="h-64">
          <TextInput accessibilityLabel="Email" />
          <Text>Hint</Text>
        </ScrollView>
      )
    }
    `,
  )
})

test('conditional utilities and dynamic class names type-check', () => {
  assertClean(
    `
    import { View, Text } from '@hozo/core'
    import { cn } from 'clsx'
    export function Row() {
      return (
        <View className={cn('p-4', active && 'bg-blue-500')}>
          <Text className="md:hover:text-white dark:text-slate-200">Hi</Text>
        </View>
      )
    }
    `,
    ['cn', 'active'],
  )
})

// Without this the suite above could pass by the harness never reporting
// anything -- the same way a stylesheet check passes when the stylesheet
// is empty.
test('the harness reports a genuine type error', () => {
  const errors = typeCheckWeb([{ name: 'Bad', jsx: `<div tabIndex="0">x</div>` }])
  assert.equal(errors.length, 1)
  assert.match(errors[0]!.line, /Type 'string' is not assignable to type 'number'/)
})

test('a free identifier is not itself reported', () => {
  const clean = typeCheckWeb([{ name: 'Ok', jsx: `<div onClick={save}>x</div>` }], ['save'])
  assert.deepEqual(clean, [])
  const undeclared = typeCheckWeb([{ name: 'Bad', jsx: `<div onClick={save}>x</div>` }])
  assert.equal(undeclared.length, 1)
  assert.match(undeclared[0]!.line, /Cannot find name 'save'/)
})
