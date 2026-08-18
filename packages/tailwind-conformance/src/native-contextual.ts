import { compileNative } from '@hozo/compiler'

export type ContextualVerdict = 'COVERED' | 'REFUSED' | 'SILENT'

export interface NativeContextualCase {
  candidate: 'transition' | 'duration-200' | 'ease-in-out'
  purpose: string
  className: string
  expected: string[]
}

export interface NativeContextualResult extends NativeContextualCase {
  verdict: ContextualVerdict
  detail?: string
}

export const NATIVE_CONTEXTUAL_CASES: NativeContextualCase[] = [
  {
    candidate: 'transition',
    purpose: 'default transition drives an interactive transform',
    className: 'transition hover:scale-95',
    expected: ['HozoPressable', 'duration: 150', 'transform: true'],
  },
  {
    candidate: 'duration-200',
    purpose: 'duration override drives an interactive background colour',
    className: 'bg-white transition duration-200 hover:bg-blue-500',
    expected: ['HozoPressable', 'duration: 200', 'colors: true'],
  },
  {
    candidate: 'ease-in-out',
    purpose: 'easing override reaches inherited Animated.Text colour',
    className: 'text-gray-500 transition ease-in-out hover:text-blue-500',
    expected: ["easing: 'ease-in-out'", 'HozoText', 'colors: true'],
  },
]

export function compareNativeContextual(testCase: NativeContextualCase): NativeContextualResult {
  const source =
    `import { Pressable } from '@hozo/core'\n` +
    `export function C() {\n` +
    `  return <Pressable accessibilityRole="button" className="${testCase.className}">x</Pressable>\n` +
    `}\n`
  const [result] = compileNative(source)
  if (!result) return { ...testCase, verdict: 'SILENT', detail: 'no component compiled' }
  const refusal = result.diagnostics.find((diagnostic) =>
    diagnostic.code === 'WEB_ONLY_PROPERTY_ON_NATIVE' || diagnostic.code === 'NOT_WIRED_ON_NATIVE')
  if (refusal) {
    return { ...testCase, verdict: 'REFUSED', detail: refusal.message }
  }
  const missing = testCase.expected.filter((fragment) =>
    !result.jsx.includes(fragment) && !result.runtimeImports.includes(fragment))
  if (missing.length > 0) {
    return {
      ...testCase,
      verdict: 'SILENT',
      detail: `compiled without the expected lowering markers: ${missing.join(', ')}`,
    }
  }
  return { ...testCase, verdict: 'COVERED' }
}
