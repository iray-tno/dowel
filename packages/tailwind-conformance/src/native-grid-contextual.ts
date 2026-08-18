import { compileNative } from '@hozo/compiler'

import type { ContextualVerdict } from './native-contextual.ts'

export interface NativeGridContextualCase {
  name: string
  purpose: string
  className: string
  children: string
  expected: string[]
}

export interface NativeGridContextualResult extends NativeGridContextualCase {
  verdict: ContextualVerdict
  detail?: string
}

/**
 * Grid utilities are intentionally refused in isolation: React Native has no
 * style object that can carry `display: grid`. In a grid container, however,
 * the compiler lowers the container and its item metadata together to the
 * Hozo solver. These cases measure that contextual contract separately from
 * the one-utility Native coverage number.
 */
export const NATIVE_GRID_CONTEXTUAL_CASES: NativeGridContextualCase[] = [
  {
    name: 'minimum fractional columns',
    purpose: 'fixed minimums reserve space before fractional distribution without measurement',
    className: 'grid grid-cols-[minmax(120px,2fr)_1fr]',
    children: '<View /><View />',
    expected: ["{ kind: 'minmax', min: 120, value: 2 }", "{ kind: 'fr', value: 1 }"],
  },
  {
    name: 'responsive columns and gap',
    purpose: 'breakpoint changes replace tracks without selecting measured layout',
    className: 'grid grid-cols-1 gap-2 md:grid-cols-3 md:gap-4',
    children: '<View /><View /><View />',
    expected: [
      'useHozoBreakpoint',
      '__hozoBp_md ?',
      'columnGap={__hozoBp_md ? 16 : (8)}',
      'rowGap={__hozoBp_md ? 16 : (8)}',
    ],
  },
  {
    name: 'equal columns and gap',
    purpose: 'ordinary equal-column grid selects the measurement-free renderer',
    className: 'grid grid-cols-3 gap-4',
    children: '<View /><View /><View /><View />',
    expected: [
      'HozoGrid',
      "{ kind: 'fr', value: 1 }",
      'columnGap={16}',
      'rowGap={16}',
    ],
  },
  {
    name: 'mixed fixed and fractional columns',
    purpose: 'simple arbitrary tracks retain their fixed/fr proportions',
    className: 'grid grid-cols-[120px_2fr_1fr]',
    children: '<View /><View /><View />',
    expected: [
      "{ kind: 'points', value: 120 }",
      "{ kind: 'fr', value: 2 }",
      "{ kind: 'fr', value: 1 }",
    ],
  },
  {
    name: 'column placement',
    purpose: 'column lines and spans become solver item metadata',
    className: 'grid grid-cols-3',
    children: '<View className="col-start-2 col-end-4" /><View />',
    expected: ['HozoGridItem', 'columnSpan={2}', 'columnStart={1}'],
  },
  {
    name: 'measured row span',
    purpose: 'row spans opt into the measured two-dimensional renderer',
    className: 'grid grid-cols-2 gap-2',
    children: '<View className="row-span-2" /><View /><View />',
    expected: ['HozoGridItem', 'rowSpan={2}'],
  },
  {
    name: 'explicit rows and full span',
    purpose: 'explicit row tracks resolve full and negative-line placement',
    className: 'grid grid-cols-2 grid-rows-3',
    children: '<View className="row-span-full" /><View className="row-start-2 row-end--1" />',
    expected: [
      "rowTracks={[{ kind: 'fr', value: 1 }",
      'rowSpan={3}',
      'rowStart={1}',
    ],
  },
]

export function compareNativeGridContextual(
  testCase: NativeGridContextualCase,
): NativeGridContextualResult {
  const source =
    `import { View } from '@hozo/core'\n` +
    `export function C() {\n` +
    `  return <View className="${testCase.className}">${testCase.children}</View>\n` +
    `}\n`
  const [result] = compileNative(source)
  if (!result) return { ...testCase, verdict: 'SILENT', detail: 'no component compiled' }
  const refusal = result.diagnostics.find((diagnostic) =>
    diagnostic.code === 'WEB_ONLY_PROPERTY_ON_NATIVE' || diagnostic.code === 'NOT_WIRED_ON_NATIVE')
  if (refusal) return { ...testCase, verdict: 'REFUSED', detail: refusal.message }
  const missing = testCase.expected.filter((fragment) =>
    !result.jsx.includes(fragment) && !result.runtimeImports.includes(fragment))
  if (missing.length > 0) {
    return {
      ...testCase,
      verdict: 'SILENT',
      detail: `compiled without the expected grid lowering markers: ${missing.join(', ')}`,
    }
  }
  return { ...testCase, verdict: 'COVERED' }
}
