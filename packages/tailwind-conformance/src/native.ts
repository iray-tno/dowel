// Native-side coverage. Deliberately *not* a fidelity check: Tailwind is
// CSS, so it can only be ground truth for the Web lowering. Nothing
// authoritative defines what `p-4` should be in React Native, so there's
// nothing to diff against -- only what Dowel does with each utility.
//
// The distinction that matters here is between a gap Dowel *knows about*
// and one it doesn't. Refusing `grid` by name is a supportable answer;
// quietly emitting nothing is the failure mode this project keeps trying
// to avoid, so the two are counted separately rather than lumped together
// as "unsupported".

import { compileNative } from '@dowel/compiler'

export type NativeVerdict = 'COVERED' | 'REFUSED' | 'SILENT'

export interface NativeComparison {
  candidate: string
  verdict: NativeVerdict
  detail?: string
}

const WEB_ONLY = 'WEB_ONLY_PROPERTY_ON_NATIVE'

/// Whether a utility works can depend on which primitive it's applied to:
/// truncation lowers to `numberOfLines`, which only exists on `Text`. So
/// each candidate is tried on both, and counts as covered if *either*
/// works -- the question being asked is "can this be used on Native at
/// all", not "does it work on a View".
const PROBE_PRIMITIVES = ['View', 'Text'] as const

function probe(candidate: string, primitive: string): NativeComparison {
  const source =
    `import { ${primitive} } from '@dowel/core'\n` +
    `const el = <${primitive} className="${candidate}">x</${primitive}>\n`
  const results = compileNative(source)
  if (results.length === 0) {
    return { candidate, verdict: 'SILENT', detail: 'no component compiled' }
  }

  const [result] = results
  const refusals = result.diagnostics.filter((d) => d.code === WEB_ONLY)

  // A refusal is checked first, and beats any partial output: it's a
  // build-stopping error, so the utility can't be used on Native at all
  // even if some of what it expands to did lower. `truncate` on a View is
  // exactly that -- its `overflow` lowers fine while the truncation itself
  // has nowhere to go, and calling it "covered" would claim a build that
  // in fact fails.
  if (refusals.length > 0) {
    return { candidate, verdict: 'REFUSED', detail: refusals[0].message }
  }
  // A prop counts as coverage too: RN expresses some CSS concepts that
  // way (`numberOfLines`), and the utility is honoured either way.
  const emitsProp = /\s\w+=[{"]/.test(result.jsx.replace(/\sstyle=\{[^}]*\}+/g, ''))
  // Style entries are `key: value,` lines inside the generated object.
  const emitsStyle = /^\s+\w+:\s*\S/m.test(result.styles.replace(/^\s*dowel\w*:\s*\{$/gm, ''))
  if (emitsStyle || emitsProp) {
    return { candidate, verdict: 'COVERED' }
  }
  return {
    candidate,
    verdict: 'SILENT',
    detail: 'compiles to no style and raises no diagnostic',
  }
}

export function compareNativeCandidate(candidate: string): NativeComparison {
  const attempts = PROBE_PRIMITIVES.map((primitive) => ({
    primitive,
    result: probe(candidate, primitive),
  }))

  const working = attempts.find((a) => a.result.verdict === 'COVERED')
  if (working) {
    return working.primitive === 'View'
      ? working.result
      : { ...working.result, detail: `only on ${working.primitive}` }
  }
  // Otherwise report the more informative verdict: a refusal names the
  // reason, silence doesn't.
  return attempts.find((a) => a.result.verdict === 'REFUSED')?.result ?? attempts[0].result
}
