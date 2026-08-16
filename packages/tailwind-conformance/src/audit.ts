// Audits Dowel's Native refusals against React Native's own type surface.
//
// `unsupported_on_native()` refuses thousands of utilities, and until now
// every one of those refusals was unchecked: Dowel decided what React Native
// can't do, and then Dowel's report counted its own decisions as the
// denominator. A wrong refusal is invisible that way -- it removes a utility
// from the numerator *and* the denominator at once, so the percentage never
// moves and nothing ever points at it.
//
// The check is the same move the full Tailwind catalogue made on the Web
// side: ask the other tool to enumerate itself. For each refused utility,
// take the CSS Tailwind says it produces, and ask React Native's `.d.ts`
// whether it could hold that. If it could, the refusal is suspect and wants
// a human read.
//
// A suspect verdict is not proof of a bug. React Native's types are a
// necessary condition, not a sufficient one -- `filter` is typed but only
// some functions are implemented, and a native style key can exist while
// doing nothing on one platform. Suspect means "this refusal is a claim we
// can no longer support from the types alone", which is exactly the set
// worth reading.

import { normalize } from './normalize.ts'
import { camelCase, reactNativeCssProperties, reactNativeStyleKeys } from './native-surface.ts'

export type RefusalVerdict = 'CONFIRMED' | 'SUSPECT' | 'PARTIAL' | 'UNCHECKABLE'

export interface RefusalAudit {
  candidate: string
  verdict: RefusalVerdict
  /** CSS properties React Native could hold. */
  expressible: string[]
  /** CSS properties it could not, with the reason. */
  inexpressible: string[]
  /** Of the expressible ones, those react-native-css also converts. */
  alsoInReactNativeCss: string[]
}

/**
 * Whether React Native could hold this declaration, and why not if it
 * couldn't.
 */
function expressible(property: string, value: string): { ok: boolean; reason?: string } {
  const keys = surface()
  const key = keys.get(camelCase(property))
  if (key === undefined) return { ok: false, reason: 'no such style key' }
  // A closed union of string literals is the only case where the type says
  // anything useful about values. Everything else -- numbers, colours,
  // dimensions -- is a conversion question Dowel already answers elsewhere,
  // not an expressibility one.
  const text = value.trim()
  const numeric = /^-?[\d.]+(px)?$/.test(text)
  const percent = /^-?[\d.]+%$/.test(text)
  // Anything the type admits: a listed keyword, a number where numbers go,
  // a percentage where `DimensionValue` allows one.
  if (key.values?.has(text)) return { ok: true }
  if (key.numeric && numeric) return { ok: true }
  if (key.percent && percent) return { ok: true }
  // Otherwise, if the type said anything at all, it said no.
  if (key.values || key.numeric || key.percent) {
    return { ok: false, reason: `${key.name} does not accept \`${text}\`` }
  }
  return { ok: true }
}

/**
 * Refusals the types cannot adjudicate, with the reason they are right
 * anyway.
 *
 * The counterpart of `ACCEPTED_DIFFERENCES` in `compare.ts`, and it exists
 * for the same reason: a standing entry needs a justification, not just a
 * line. React Native's types are a necessary condition and not a sufficient
 * one, so a handful of correct refusals will always look suspect -- and
 * leaving them in the suspect count would make the number mean "unreviewed"
 * rather than "unsupported", which is the thing that lets a wrong refusal
 * hide again.
 */
const ACKNOWLEDGED_REFUSALS: Record<string, string> = {
  'bg-none': `\`backgroundImage\` is typed \`ReadonlyArray<BackgroundImageValue> | string\`,
    which a string can satisfy -- but React Native's value there is a list of
    gradients, and \`none\` is not one of them. Dowel emits no gradients on
    Native either, so there is nothing for this to clear.`,
  'aspect-auto': `\`aspectRatio\` is typed \`number | string\`, so the types can't rule out
    \`auto\` -- but React Native has no auto aspect ratio, and passing the
    string makes it ignore the style rather than fall back to content size.`,
}

let cachedSurface: ReturnType<typeof reactNativeStyleKeys> | undefined
function surface() {
  cachedSurface ??= reactNativeStyleKeys()
  return cachedSurface
}

let cachedRnc: Set<string> | undefined | null = null
function rncProperties(): Set<string> | undefined {
  if (cachedRnc === null) cachedRnc = reactNativeCssProperties()
  return cachedRnc
}

/**
 * Audits one refusal against the CSS Tailwind produces for it.
 *
 * `oracleBlock` is Tailwind's declaration block for the candidate -- the
 * same text the Web comparison uses, so the audit is asking about the
 * utility's actual meaning rather than about its name.
 */
export function auditRefusal(
  candidate: string,
  oracleBlock: string | undefined,
  vars: Map<string, string>,
): RefusalAudit {
  const base: RefusalAudit = {
    candidate,
    verdict: 'UNCHECKABLE',
    expressible: [],
    inexpressible: [],
    alsoInReactNativeCss: [],
  }
  if (!oracleBlock) return base

  const resolved = normalize(oracleBlock, vars)
  // An unresolved value doesn't stop the property-level question, and the
  // property is most of the signal here -- `no such style key` needs no
  // value at all.
  const declarations = [...resolved.declarations].filter(([property]) => !property.startsWith('--'))
  if (declarations.length === 0) return base

  const rnc = rncProperties()
  for (const [property, value] of declarations) {
    const check = expressible(property, value)
    if (check.ok) {
      base.expressible.push(property)
      if (rnc?.has(property)) base.alsoInReactNativeCss.push(property)
    } else {
      base.inexpressible.push(`${property} (${check.reason})`)
    }
  }

  if (base.expressible.length === 0) base.verdict = 'CONFIRMED'
  else if (base.inexpressible.length === 0) base.verdict = 'SUSPECT'
  else base.verdict = 'PARTIAL'

  const acknowledged = ACKNOWLEDGED_REFUSALS[candidate]
  if (acknowledged && base.verdict === 'SUSPECT') {
    base.verdict = 'CONFIRMED'
    base.inexpressible.push(`reviewed: ${acknowledged.replace(/\s+/g, ' ').trim()}`)
  }
  return base
}
