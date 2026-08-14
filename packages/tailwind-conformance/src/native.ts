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
  /**
   * Set when a utility lowers on some primitives but is refused on
   * others. Counting it as covered answers "is this usable on Native",
   * but writing it on the wrong element still fails the build -- so the
   * restriction has to be reported, not folded into the number.
   */
  restrictedTo?: string[]
}

/// Diagnostic codes that mean "Dowel knows it can't render this, and says
/// so". Both count as REFUSED, since the distinction this report draws is
/// named-gap vs. silent-gap, not error vs. warning:
/// - WEB_ONLY: impossible on the platform (Yoga has no grid).
/// - VARIANT_NOT_WIRED: possible, not built yet (`dark:`, breakpoints).
const NAMED_GAPS = new Set(['WEB_ONLY_PROPERTY_ON_NATIVE', 'VARIANT_NOT_WIRED_ON_NATIVE'])

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
  const refusals = result.diagnostics.filter((d) => NAMED_GAPS.has(d.code))

  // A refusal is checked first, and beats any partial output: it's a
  // build-stopping error, so the utility can't be used on Native at all
  // even if some of what it expands to did lower. `truncate` on a View is
  // exactly that -- its `overflow` lowers fine while the truncation itself
  // has nowhere to go, and calling it "covered" would claim a build that
  // in fact fails.
  if (refusals.length > 0) {
    return {
      candidate,
      verdict: 'REFUSED',
      detail: `[${refusals[0].severity}] ${refusals[0].message}`,
    }
  }
  // A prop counts as coverage too: RN expresses some CSS concepts that
  // way (`numberOfLines`), and the utility is honoured either way.
  const emitsProp = /\s\w+=[{"]/.test(result.jsx.replace(/\sstyle=\{[^}]*\}+/g, ''))

  // A style entry counts only if it has declarations in it *and* the
  // rendered JSX references it. Both halves are load-bearing, and each was
  // wrong on its own:
  // - checking the StyleSheet alone (until 2026-08-15) scored every
  //   variant-prefixed utility as covered -- `hover:bg-blue-500` does
  //   produce a `dowel0_hover` entry, it just never reaches the element.
  // - checking the reference alone lets `whitespace-normal` through, which
  //   emits an empty `dowel0: {}` and a `style` prop pointing at it.
  const entries = [...result.styles.matchAll(/^ {2}(\w+):\s*\{\n([\s\S]*?)^ {2}\},/gm)]
  const nonEmpty = entries.filter((m) => m[2].trim() !== '').map((m) => m[1])
  const unreferenced = nonEmpty.filter((name) => !result.jsx.includes(`styles.${name}`))
  const rendered = nonEmpty.filter((name) => result.jsx.includes(`styles.${name}`))

  if (rendered.length > 0 || emitsProp) {
    return { candidate, verdict: 'COVERED' }
  }
  return {
    candidate,
    verdict: 'SILENT',
    detail: unreferenced.length
      ? `compiles to a style (${unreferenced.join(', ')}) that the JSX never references, and raises no diagnostic`
      : 'compiles to no style and raises no diagnostic',
  }
}

export function compareNativeCandidate(candidate: string): NativeComparison {
  const attempts = PROBE_PRIMITIVES.map((primitive) => ({
    primitive,
    result: probe(candidate, primitive),
  }))

  const working = attempts.filter((a) => a.result.verdict === 'COVERED')
  if (working.length > 0) {
    const covered = working[0].result
    if (working.length === PROBE_PRIMITIVES.length) {
      return covered
    }
    // Works somewhere but not everywhere. Still covered, but the report
    // must say where -- otherwise the number quietly implies it works on
    // any element, and using it on the wrong one is a build failure.
    return { ...covered, restrictedTo: working.map((a) => a.primitive) }
  }
  // Otherwise report the more informative verdict: a refusal names the
  // reason, silence doesn't.
  return attempts.find((a) => a.result.verdict === 'REFUSED')?.result ?? attempts[0].result
}
