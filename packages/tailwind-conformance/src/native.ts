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

export function compareNativeCandidate(candidate: string): NativeComparison {
  const source = `import { View } from '@dowel/core'\nconst el = <View className="${candidate}" />\n`
  const results = compileNative(source)
  if (results.length === 0) {
    return { candidate, verdict: 'SILENT', detail: 'no component compiled' }
  }

  const [result] = results
  const refusals = result.diagnostics.filter((d) => d.code === WEB_ONLY)

  // A refusal is checked first, and beats any partial output: it's a
  // build-stopping error, so the utility can't be used on Native at all
  // even if some of what it expands to did lower. `truncate` is exactly
  // that case -- its `overflow` lowers fine while its `text-overflow`
  // can't, and calling it "covered" would claim a build that in fact
  // fails.
  if (refusals.length > 0) {
    return { candidate, verdict: 'REFUSED', detail: refusals[0].message }
  }
  // Style entries are `key: value,` lines inside the generated object.
  if (/^\s+\w+:\s*\S/m.test(result.styles.replace(/^\s*dowel\w*:\s*\{$/gm, ''))) {
    return { candidate, verdict: 'COVERED' }
  }
  return {
    candidate,
    verdict: 'SILENT',
    detail: 'compiles to no style and raises no diagnostic',
  }
}
