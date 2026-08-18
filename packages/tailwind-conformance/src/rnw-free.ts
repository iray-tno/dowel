import { compile, compileNative } from '@dowel/compiler'

interface RnwFreeCase {
  primitive: string
  jsx: string
  webMarker: string
  nativeMarker: string
}

export interface RnwFreeResult extends RnwFreeCase {
  covered: boolean
  detail?: string
}

export const RNW_FREE_CASES: RnwFreeCase[] = [
  { primitive: 'View', jsx: '<View />', webMarker: '<div', nativeMarker: '<View' },
  { primitive: 'Text', jsx: '<Text>Hello</Text>', webMarker: '<span', nativeMarker: '<Text' },
  { primitive: 'Paragraph', jsx: '<Paragraph>Body</Paragraph>', webMarker: '<p', nativeMarker: '<Text' },
  { primitive: 'Heading', jsx: '<Heading level={2}>Title</Heading>', webMarker: '<h2', nativeMarker: '<Text' },
  { primitive: 'Section', jsx: '<Section />', webMarker: '<section', nativeMarker: '<View' },
  { primitive: 'Article', jsx: '<Article />', webMarker: '<article', nativeMarker: 'role="article"' },
  { primitive: 'Nav', jsx: '<Nav accessibilityLabel="Primary" />', webMarker: '<nav', nativeMarker: 'role="navigation"' },
  { primitive: 'List', jsx: '<List><ListItem>One</ListItem></List>', webMarker: '<ul', nativeMarker: 'accessibilityRole="list"' },
  { primitive: 'ListItem', jsx: '<ListItem>One</ListItem>', webMarker: '<li', nativeMarker: 'role="listitem"' },
  { primitive: 'Button', jsx: '<Button>Save</Button>', webMarker: '<button', nativeMarker: '<Pressable' },
  {
    primitive: 'FlatList',
    jsx: '<FlatList data={rows} renderItem={({ item }) => <Text className="p-2">{item}</Text>} />',
    webMarker: '<FlatList',
    nativeMarker: '<FlatList',
  },
  {
    primitive: 'ScrollView',
    jsx: '<ScrollView horizontal className="h-40"><View /></ScrollView>',
    webMarker: '<div',
    nativeMarker: '<ScrollView',
  },
  {
    primitive: 'Image',
    jsx: '<Image src="https://example.com/cover.jpg" alt="Cover" className="w-20 h-20 object-cover" />',
    webMarker: '<img',
    nativeMarker: '<Image',
  },
  {
    primitive: 'Pressable',
    jsx: '<Pressable accessibilityRole="button">Open</Pressable>',
    webMarker: '<div',
    nativeMarker: '<Pressable',
  },
  {
    primitive: 'Link',
    jsx: '<Link href="https://example.com">Docs</Link>',
    webMarker: '<a',
    nativeMarker: '<DowelLink',
  },
  {
    primitive: 'TextInput',
    jsx: '<TextInput accessibilityLabel="Email" />',
    webMarker: '<input',
    nativeMarker: '<TextInput',
  },
  {
    primitive: 'Dialog',
    jsx: '<Dialog open={showing} onClose={dismiss} accessibilityLabel="Confirm" />',
    webMarker: '<DowelDialog',
    nativeMarker: '<DowelDialog',
  },
]

export function compareRnwFree(testCase: RnwFreeCase): RnwFreeResult {
  const source =
    `import { View, Text, Paragraph, Heading, Section, Article, Nav, List, ListItem, Button, Pressable, Link, TextInput, Dialog, Image, ScrollView, FlatList } from '@dowel/core'\n` +
    `export function C() { return ${testCase.jsx} }\n`
  const [web] = compile(source)
  const [native] = compileNative(source)
  if (!web || !native) return { ...testCase, covered: false, detail: 'one backend emitted no component' }
  const failures: string[] = []
  if (!web.jsx.includes(testCase.webMarker)) failures.push(`Web marker ${testCase.webMarker}`)
  if (!native.jsx.includes(testCase.nativeMarker)) failures.push(`Native marker ${testCase.nativeMarker}`)
  const combined = `${web.jsx}\n${native.jsx}\n${native.runtimeImports.join('\n')}`
  if (/react-native-web|from ['"]react-native['"]/.test(combined)) {
    failures.push('backend output contains a compatibility-layer import')
  }
  if (web.diagnostics.length > 0) failures.push(`Web diagnostic ${web.diagnostics[0].code}`)
  if (native.diagnostics.length > 0) failures.push(`Native diagnostic ${native.diagnostics[0].code}`)
  return failures.length === 0
    ? { ...testCase, covered: true }
    : { ...testCase, covered: false, detail: failures.join(', ') }
}
