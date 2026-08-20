import { compile, compileNative } from '@hozo/compiler'

export interface A11yContextualCase {
  name: string
  purpose: string
  source: string
  web: string[]
  native: string[]
  diagnostics?: string[]
}

export interface A11yContextualResult extends A11yContextualCase {
  covered: boolean
  detail?: string
}

export const A11Y_CONTEXTUAL_CASES: A11yContextualCase[] = [
  {
    name: 'semantic document structure',
    purpose: 'paragraph, heading level and section intent survive platform lowering',
    source: '<Section><Heading level={2}>Title</Heading><Paragraph>Body</Paragraph></Section>',
    web: ['<section>', '<h2>Title</h2>', '<p>Body</p>'],
    native: ['<View>', '<Text accessibilityRole="header">Title</Text>', '<Text>Body</Text>'],
  },
  {
    name: 'document landmarks',
    purpose: 'article and named navigation landmarks remain explicit on both platforms',
    source: '<Article><Nav accessibilityLabel="Primary" /></Article>',
    web: ['<article>', '<nav aria-label={"Primary"}>'],
    native: ['<View role="article">', '<View role="navigation" accessibilityLabel={"Primary"}>'],
  },
  {
    name: 'invalid document nesting diagnostic',
    purpose: 'a statically invalid paragraph structure never ships silently',
    source: '<Paragraph>Intro<Section>Details</Section></Paragraph>',
    web: ['<p>Intro<section>Details</section></p>'],
    native: ['<Text>Intro<View><Text>Details</Text></View></Text>'],
    diagnostics: ['INVALID_SEMANTIC_NESTING'],
  },
  {
    name: 'ordered static list',
    purpose: 'ordered list and item semantics survive without virtualizing a small document list',
    source: '<List ordered><ListItem>First</ListItem><ListItem>Second</ListItem></List>',
    web: ['<ol>', '<li>First</li>', '<li>Second</li>'],
    native: ['<View accessibilityRole="list">', '<View role="listitem"><Text>First</Text></View>'],
  },
  {
    name: 'described Image',
    purpose: 'one alternative text input reaches native semantics on both platforms',
    source: '<Image src="https://example.com/cover.jpg" alt="Cover art" />',
    web: ['<img', 'src={"https://example.com/cover.jpg"}', 'alt={"Cover art"}'],
    native: ['<Image', 'source={{ uri: "https://example.com/cover.jpg" }}', 'accessibilityLabel={"Cover art"}'],
  },
  {
    name: 'semantic Link',
    purpose: 'a destination remains an anchor on Web and an opening link interaction on Native',
    source: '<Link href="https://example.com" accessibilityLabel="Documentation">Docs</Link>',
    web: ['<a', 'href="https://example.com"', 'aria-label={"Documentation"}'],
    native: ['<HozoLink', 'href="https://example.com"', 'accessibilityLabel={"Documentation"}'],
  },
  {
    name: 'semantic Button',
    purpose: 'name, hint and disabled state retain native semantics on both platforms',
    source: '<Button disabled={busy} accessibilityLabel="Save" accessibilityHint="Saves the draft">Save</Button>',
    web: ['<button', 'disabled={busy}', 'aria-label={"Save"}', 'aria-description={"Saves the draft"}'],
    native: [
      '<Pressable',
      'accessibilityRole="button"',
      'disabled={busy}',
      'accessibilityState={{ disabled: Boolean(busy) }}',
      'accessibilityHint={"Saves the draft"}',
    ],
  },
  {
    name: 'role-bearing Pressable',
    purpose: 'a generic interaction stays focusable and explicitly named',
    source: '<Pressable onPress={go} accessibilityRole="link" accessibilityLabel="Account">Account</Pressable>',
    // `tabIndex={0}`, not `tabIndex="0"`. This expectation held the string
    // form until `typecheck-web.test.ts` existed, which is to say the test
    // was pinning the bug: React types `tabIndex` as a `number`, and Hozo's
    // output lands in the author's own `.tsx` where their `tsc` sees it.
    web: [
      '<div',
      'role="link"',
      'tabIndex={0}',
      'aria-label={"Account"}',
      'onClick={go}',
      // Hozo put this in the tab order, so Hozo owes it Enter and Space.
      'onKeyDown={hozoActivateKeyDown}',
      'onKeyUp={hozoActivateKeyUp}',
    ],
    // `role`, not `accessibilityRole`: React Native has taken the ARIA
    // spelling since 0.71, so the two platforms now write the same word.
    native: ['<Pressable', 'role="link"', 'accessibilityLabel={"Account"}', 'onPress={go}'],
  },
  {
    name: 'named TextInput',
    purpose: 'a field name and supplemental guidance use each platform spelling',
    source: '<TextInput accessibilityLabel="Email" accessibilityHint="Work address" placeholder="you@example.com" />',
    web: ['<input', 'aria-label={"Email"}', 'aria-description={"Work address"}', 'placeholder="you@example.com"'],
    native: ['<TextInput', 'accessibilityLabel={"Email"}', 'accessibilityHint={"Work address"}', 'placeholder="you@example.com"'],
  },
  {
    name: 'modal Dialog',
    purpose: 'the accessible name, hint and dismissal callback reach the modal runtime',
    source: '<Dialog open={showing} onClose={dismiss} accessibilityLabel="Confirm" accessibilityHint="Review before continuing" />',
    web: ['<HozoDialog', 'open={showing}', 'onClose={dismiss}', 'accessibilityLabel={"Confirm"}', 'accessibilityHint={"Review before continuing"}'],
    native: ['<HozoDialog', 'open={showing}', 'onClose={dismiss}', 'accessibilityLabel={"Confirm"}', 'accessibilityHint={"Review before continuing"}'],
  },
  {
    name: 'missing interaction role diagnostic',
    purpose: 'an interactive generic container never fails accessibility silently',
    source: '<Pressable onPress={go}>Go</Pressable>',
    web: ['<div'],
    native: ['<Pressable'],
    diagnostics: ['A11Y_INTERACTIVE_WITHOUT_ROLE'],
  },
  {
    name: 'missing field name diagnostic',
    purpose: 'a placeholder is not accepted as a text field name',
    source: '<TextInput placeholder="you@example.com" />',
    web: ['<input'],
    native: ['<TextInput'],
    diagnostics: ['A11Y_MISSING_ACCESSIBLE_NAME'],
  },
  {
    name: 'incomplete Dialog diagnostic',
    purpose: 'an unnamed modal with no dismissal route reports both defects',
    source: '<Dialog open={showing} />',
    web: ['<HozoDialog'],
    native: ['<HozoDialog'],
    diagnostics: ['A11Y_MISSING_ACCESSIBLE_NAME', 'A11Y_DIALOG_WITHOUT_DISMISS'],
  },
]

export function compareA11yContextual(testCase: A11yContextualCase): A11yContextualResult {
  const source =
    `import { Article, Button, Dialog, Heading, Image, Link, List, ListItem, Nav, Paragraph, Pressable, Section, TextInput } from '@hozo/core'\n` +
    `export function C() { return ${testCase.source} }\n`
  const [web] = compile(source)
  const [native] = compileNative(source)
  if (!web || !native) return { ...testCase, covered: false, detail: 'one backend emitted no component' }

  const failures: string[] = []
  for (const marker of testCase.web) if (!web.jsx.includes(marker)) failures.push(`Web: ${marker}`)
  for (const marker of testCase.native) if (!native.jsx.includes(marker)) failures.push(`Native: ${marker}`)
  const expectedDiagnostics = testCase.diagnostics ?? []
  for (const code of expectedDiagnostics) {
    if (!web.diagnostics.some((diagnostic) => diagnostic.code === code)) failures.push(`Web diagnostic: ${code}`)
    if (!native.diagnostics.some((diagnostic) => diagnostic.code === code)) failures.push(`Native diagnostic: ${code}`)
  }
  if (expectedDiagnostics.length === 0) {
    if (web.diagnostics.length > 0) failures.push(`unexpected Web diagnostic: ${web.diagnostics[0].code}`)
    if (native.diagnostics.length > 0) failures.push(`unexpected Native diagnostic: ${native.diagnostics[0].code}`)
  }
  return failures.length === 0
    ? { ...testCase, covered: true }
    : { ...testCase, covered: false, detail: failures.join(', ') }
}
