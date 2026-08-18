import assert from 'node:assert/strict'
import { test } from 'node:test'

import { compile } from '@dowel/compiler'
import { classesDefinedIn, renderWeb } from './render.ts'

/** Compiles one source and renders the first component it produced. */
function round(source: string, scope: Record<string, unknown> = {}) {
  const [compiled] = compile(source)
  const [rendered] = renderWeb([{ name: 'C', jsx: compiled.jsx }], scope)
  return { compiled, rendered }
}

test('a compiled component mounts and produces the expected markup', () => {
  // The check nothing in this package made until 2026-08-16: every other
  // comparison here is between strings, and none of them establishes that
  // the generated JSX parses, let alone renders.
  const { rendered } = round(`
    import { View, Text } from '@dowel/core'
    export function Card() {
      return <View className="p-4"><Text className="text-xl">Hello</Text></View>
    }
  `)
  assert.equal(
    rendered.html,
    '<div class="dowel-view dowel-0"><span class="dowel-1">Hello</span></div>',
  )
})

test('semantic primitives render native document elements', () => {
  const { rendered } = round(`
    import { Section, Heading, Paragraph } from '@dowel/core'
    export function Article() {
      return <Section><Heading level={3}>Title</Heading><Paragraph>Body</Paragraph></Section>
    }
  `)
  assert.equal(rendered.html, '<section><h3>Title</h3><p>Body</p></section>')
})

test('article and navigation landmarks survive an actual Web render', () => {
  const { rendered } = round(`
    import { Article, Nav } from '@dowel/core'
    export function Shell() {
      return <Article><Nav accessibilityLabel="Primary" /></Article>
    }
  `)
  assert.equal(rendered.html, '<article><nav aria-label="Primary"></nav></article>')
})

test('every class in the DOM has a rule in the stylesheet', () => {
  // The two halves of the Web output have to agree, and nothing compared
  // them before: a class that reaches the element and matches no rule is a
  // style that silently never applies. This found the opposite -- classes
  // emitted for elements with no declarations at all.
  const { compiled, rendered } = round(
    `
    import { View, Text, Button } from '@dowel/core'
    export function Card() {
      return (
        <View className="p-4 bg-blue-500">
          <Text className="text-xl">Hello</Text>
          <Button onPress={save}>Save</Button>
          <View />
        </View>
      )
    }
    `,
    { save: () => {} },
  )
  const defined = classesDefinedIn(compiled.css)
  const undefinedClasses = [...rendered.classes].filter((name) => !defined.has(name))
  assert.deepEqual(undefinedClasses, [])
})

test('an element with no declarations carries no class at all', () => {
  // Not merely an unused class -- no attribute. It was bytes in every
  // render of every unstyled element, matching nothing.
  const { rendered } = round(`
    import { Text } from '@dowel/core'
    export function Bare() {
      return <Text>plain</Text>
    }
  `)
  assert.equal(rendered.html, '<span>plain</span>')
})

test('a View keeps its base class even with nothing else on it', () => {
  // `dowel-view` is View's own semantics rather than a compiled utility
  // (proposal §8.1), so dropping it with the rest would change the layout.
  const { rendered } = round(`
    import { View } from '@dowel/core'
    export function Bare() {
      return <View />
    }
  `)
  assert.equal(rendered.html, '<div class="dowel-view"></div>')
})

test('Image renders a semantic img with its universal source and alternative', () => {
  const { rendered } = round(`
    import { Image } from '@dowel/core'
    export function Cover() {
      return <Image className="w-20 h-20 object-cover" src="https://example.com/cover.jpg" alt="Cover" />
    }
  `)
  assert.equal(
    rendered.html,
    '<link rel="preload" as="image" href="https://example.com/cover.jpg"/>' +
      '<img class="dowel-0" src="https://example.com/cover.jpg" alt="Cover"/>',
  )
})

test('ScrollView owns only its viewport axis while its child owns content layout', () => {
  const { compiled, rendered } = round(`
    import { ScrollView, View, Text } from '@dowel/core'
    export function Rail() {
      return (
        <ScrollView horizontal className="h-40">
          <View className="flex-row gap-4"><Text>One</Text><Text>Two</Text></View>
        </ScrollView>
      )
    }
  `)
  assert.match(rendered.html, /^<div class="dowel-scroll-view dowel-0" data-dowel-horizontal="">/)
  assert.match(compiled.css, /\.dowel-scroll-view \{[\s\S]*overflow-y: auto/)
  assert.match(compiled.css, /\.dowel-scroll-view\[data-dowel-horizontal\] \{[\s\S]*overflow-x: auto/)
})

test('text kept its spacing around an interpolation', () => {
  // JSX whitespace rules, checked through the DOM rather than through the
  // emitted string -- `Hello {name}` losing its space is invisible in a
  // comparison that trims.
  const { rendered } = round(
    `
    import { Text } from '@dowel/core'
    export function Greeting() {
      return <Text className="text-xl">Hello {name}</Text>
    }
    `,
    { name: 'world' },
  )
  assert.equal(rendered.html, '<span class="dowel-0">Hello world</span>')
})
