// Normalizes a CSS declaration block into a canonical longhand map, so
// Tailwind's output and Dowel's can be compared by meaning rather than by
// spelling. The two differ constantly without disagreeing:
//
//   flex: 1                        vs  flex: 1 1 0%
//   padding: calc(var(--spacing)*4) vs  padding-top: 16px; ...(4 longhands)
//   background-color: var(--color-blue-500) vs background-color: oklch(...)
//   line-height: calc(1.75 / 1.25)  vs  line-height: 28px
//
// Anything this can't confidently resolve is reported as unresolvable
// rather than guessed at -- a normalizer that quietly mis-resolves would
// produce false matches and false diffs, which is worse than a smaller
// comparable set.

const ROOT_FONT_SIZE_PX = 16

export interface Normalized {
  declarations: Map<string, string>
  /// Raw text of anything the normalizer declined to interpret. Non-empty
  /// means the comparison for this rule is not trustworthy.
  unresolved: string[]
}

/** Splits `a: b; c: d;` into pairs, ignoring empties. */
function splitDeclarations(block: string): Array<[string, string]> {
  const out: Array<[string, string]> = []
  let depth = 0
  let current = ''
  for (const ch of block) {
    if (ch === '(') depth++
    if (ch === ')') depth--
    if (ch === ';' && depth === 0) {
      if (current.trim()) out.push(splitOne(current))
      current = ''
      continue
    }
    current += ch
  }
  if (current.trim()) out.push(splitOne(current))
  return out.filter((pair): pair is [string, string] => pair !== null && pair[0] !== '')
}

function splitOne(decl: string): [string, string] {
  const idx = decl.indexOf(':')
  if (idx === -1) return ['', '']
  return [decl.slice(0, idx).trim().toLowerCase(), decl.slice(idx + 1).trim()]
}

/**
 * Substitutes `var(--x)` / `var(--x, fallback)` using `vars`.
 * `--tw-*` properties are Tailwind's own runtime registers, declared via
 * `@property` with initial values; the two that matter for the utilities
 * Dowel supports are resolved here explicitly, and anything else `--tw-*`
 * is left in place so the caller marks the rule unresolved.
 */
function resolveVars(value: string, vars: Map<string, string>, depth = 0): string {
  // Generous, because each pass resolves only the first `var()` and a
  // single declaration can chain many: Tailwind's `filter` alone splices in
  // nine. The cap is here to stop a self-referential custom property from
  // looping forever, not to bound normal nesting -- an earlier limit of 8
  // silently left the tail of that `filter` unresolved.
  if (depth > 100) return value
  const re = /var\(\s*(--[a-z0-9-]+)\s*(?:,\s*([^]*?)\s*)?\)/i
  const match = re.exec(value)
  if (!match) return value

  const [full, name, fallback] = match
  let replacement: string | undefined
  if (name === '--tw-border-style') {
    // Registered by Tailwind with `initial-value: solid`.
    replacement = 'solid'
  } else if (name === '--tw-leading' && fallback !== undefined) {
    // Set only by an explicit `leading-*`; absent here, so the fallback
    // (the text-size's own line-height) applies.
    replacement = fallback
  } else if (vars.has(name)) {
    replacement = vars.get(name)
  } else if (fallback !== undefined) {
    replacement = fallback
  }
  if (replacement === undefined) return value
  return resolveVars(value.replace(full, replacement), vars, depth + 1)
}

/** Evaluates `calc(...)` for the +,-,*,/ arithmetic Tailwind actually emits. */
function evaluateCalc(value: string): string {
  let out = value
  for (let i = 0; i < 8; i++) {
    const match = /calc\(([^()]*)\)/.exec(out)
    if (!match) break
    const evaluated = evaluateArithmetic(match[1])
    if (evaluated === null) return out
    out = out.replace(match[0], evaluated)
  }
  return out
}

function evaluateArithmetic(expr: string): string | null {
  if (/infinity/i.test(expr)) return 'infinity'
  // Every term must be a bare number or a px/rem length for this to be
  // safe to fold; a mix of units (or an unknown one) bails out.
  const units = new Set(
    (expr.match(/[\d.]+(px|rem|%|em|deg)/g) ?? []).map((t) => /[a-z%]+$/.exec(t)![0]),
  )
  if (units.size > 1) return null
  const unit = [...units][0] ?? ''
  // Every term shares one unit, so the arithmetic is exact whatever that
  // unit means -- `calc(-0.025em * -1)` is `0.025em` without knowing any
  // font size. `em` was excluded here until 2026-08-16, which left all six
  // `-tracking-*` unresolvable on Tailwind's side and so unmeasurable.
  // Mixing units is the unsafe case, and the check above already bails on
  // it.
  const bare = expr.replace(/(px|rem|%|deg|em)/g, '')
  if (!/^[\d\s.+\-*/()]+$/.test(bare)) return null
  let result: number
  try {
    // Arithmetic-only after the guard above.
    result = Function(`"use strict"; return (${bare})`)() as number
  } catch {
    return null
  }
  if (!Number.isFinite(result)) return null
  return unit ? `${result}${unit}` : `${result}`
}

function remToPx(value: string): string {
  return value.replace(/(-?[\d.]+)rem/g, (_, n: string) => `${parseFloat(n) * ROOT_FONT_SIZE_PX}px`)
}

/** Canonicalizes an already-resolved value's spelling. */
function canonicalizeValue(value: string): string {
  let out = value.trim().toLowerCase().replace(/\s+/g, ' ')
  out = remToPx(out)
  // 0 is 0 regardless of unit. Any unit -- the list used to be `px|rem|%`,
  // which was every unit the named catalogue could produce and three of
  // the two dozen an arbitrary value can. `space-x-[1.5em]` reported a
  // mismatch between `0em` and `0`, which are the same length.
  out = out.replace(/(^|\s)(-?0)[a-z]+(\s|$)/g, '$1$2$3')
  // Trim pointless decimals: 16.0px -> 16px
  out = out.replace(/(-?\d+)\.0+(?=px|%|\s|$)/g, '$1')
  return out.trim()
}

/**
 * Block-axis logical properties only diverge from their physical
 * counterparts under a vertical `writing-mode`. React Native has no such
 * mode, so Dowel assumes horizontal throughout and lowers `py-*` to
 * top/bottom; treating the two as equal here reflects that project-wide
 * assumption rather than papering over a difference.
 *
 * Inline-axis properties are deliberately *not* folded: start/end vs
 * left/right genuinely differ under RTL, which both target platforms
 * support.
 */
const BLOCK_AXIS_EQUIVALENTS: Record<string, string> = {
  'padding-block-start': 'padding-top',
  'padding-block-end': 'padding-bottom',
  'margin-block-start': 'margin-top',
  'margin-block-end': 'margin-bottom',
}

function canonicalizePropertyName(prop: string): string {
  return BLOCK_AXIS_EQUIVALENTS[prop] ?? prop
}

/**
 * Per-property canonicalization for values that are equivalent in CSS but
 * spelled differently. `opacity: 50%` and `opacity: 0.5` are the same
 * declaration; Tailwind writes the former, Dowel the latter.
 */
function canonicalizeProperty(prop: string, value: string): string {
  if (prop === 'opacity') {
    const pct = /^(-?[\d.]+)%$/.exec(value)
    if (pct) return canonicalizeValue(`${parseFloat(pct[1]) / 100}`)
  }
  if (prop === 'box-shadow') return dropNoOpShadowLayers(value)
  // Tailwind builds `filter` from a fixed list of slots
  // (`var(--tw-blur,) var(--tw-brightness,) ...`), most of which resolve to
  // nothing; collapse the leftover whitespace so one active filter compares
  // equal to Dowel's single function.
  if (prop === 'filter') return value.replace(/\s+/g, ' ').trim()
  return value
}

/**
 * Tailwind's `box-shadow` always splices in its ring/inset-ring registers,
 * which default to `0 0 #0000` -- fully transparent, i.e. no shadow at all.
 * Dropping them lets a rule with one real shadow compare equal to Dowel's,
 * which emits only that shadow.
 */
function dropNoOpShadowLayers(value: string): string {
  const layers = splitTopLevel(value, ',')
    .map((layer) => layer.trim())
    .filter((layer) => layer !== '' && !/^0 0 #0000$/.test(layer))
  return layers.join(', ')
}

/** Splits on `sep`, ignoring separators nested inside parentheses. */
function splitTopLevel(value: string, sep: string): string[] {
  const out: string[] = []
  let depth = 0
  let current = ''
  for (const ch of value) {
    if (ch === '(') depth++
    if (ch === ')') depth--
    if (ch === sep && depth === 0) {
      out.push(current)
      current = ''
      continue
    }
    current += ch
  }
  out.push(current)
  return out
}

const FOUR_SIDES = ['top', 'right', 'bottom', 'left'] as const

/** Expands the shorthands that appear in either side's output. */
function expandShorthand(prop: string, value: string): Array<[string, string]> {
  const sides = (prefix: string, suffix = '') => {
    const parts = value.split(/\s+/)
    // CSS 1/2/3/4-value box syntax.
    const [t, r, b, l] =
      parts.length === 1
        ? [parts[0], parts[0], parts[0], parts[0]]
        : parts.length === 2
          ? [parts[0], parts[1], parts[0], parts[1]]
          : parts.length === 3
            ? [parts[0], parts[1], parts[2], parts[1]]
            : [parts[0], parts[1], parts[2], parts[3]]
    return [
      [`${prefix}-top${suffix}`, t],
      [`${prefix}-right${suffix}`, r],
      [`${prefix}-bottom${suffix}`, b],
      [`${prefix}-left${suffix}`, l],
    ] as Array<[string, string]>
  }

  // The inline/block logical shorthands take 1 or 2 values (start, end).
  const axis = (prefix: string, suffix: string, names: [string, string]) => {
    const parts = value.split(/\s+/)
    const [start, end] = parts.length === 1 ? [parts[0], parts[0]] : parts
    return [
      [`${prefix}-${suffix}-${names[0]}`, start],
      [`${prefix}-${suffix}-${names[1]}`, end],
    ] as Array<[string, string]>
  }

  switch (prop) {
    case 'padding':
      return sides('padding')
    case 'margin':
      return sides('margin')
    case 'padding-inline':
      return axis('padding', 'inline', ['start', 'end'])
    case 'padding-block':
      return axis('padding', 'block', ['start', 'end'])
    case 'margin-inline':
      return axis('margin', 'inline', ['start', 'end'])
    case 'margin-block':
      return axis('margin', 'block', ['start', 'end'])
    case 'inset-inline':
      return axis('inset', 'inline', ['start', 'end'])
    case 'border-width':
      return sides('border', '-width')
    case 'border-style':
      return sides('border', '-style')
    case 'inset':
      return FOUR_SIDES.map((side, i) => {
        const parts = value.split(/\s+/)
        const v = parts.length === 1 ? parts[0] : parts[i] ?? parts[0]
        return [side, v] as [string, string]
      })
    case 'gap':
      return [
        ['row-gap', value],
        ['column-gap', value],
      ]
    case 'flex':
      return expandFlex(value)
    default:
      return [[prop, value]]
  }
}

/** CSS `flex` shorthand -> the three longhands, per the spec's defaults. */
function expandFlex(value: string): Array<[string, string]> {
  const v = value.trim()
  const keyword: Record<string, [string, string, string]> = {
    auto: ['1', '1', 'auto'],
    initial: ['0', '1', 'auto'],
    none: ['0', '0', 'auto'],
  }
  if (keyword[v]) {
    const [g, s, b] = keyword[v]
    return [
      ['flex-grow', g],
      ['flex-shrink', s],
      ['flex-basis', b],
    ]
  }
  const parts = v.split(/\s+/)
  const isNumber = (part: string) => /^[+-]?(\d+\.?\d*|\.\d+)$/.test(part)

  // The two-value form is ambiguous and the spec resolves it by type: a
  // number is the shrink factor, anything else is the basis. Reading it
  // positionally made Tailwind's `flex: 0 auto` (which is exactly
  // `flex: 0 1 auto`) look like a mismatch against Dowel's longhand.
  const grow = parts[0]
  let shrink = '1'
  let basis = '0%'
  if (parts.length === 2) {
    if (isNumber(parts[1])) shrink = parts[1]
    else basis = parts[1]
  } else if (parts.length >= 3) {
    shrink = parts[1]
    basis = parts[2]
  }
  return [
    ['flex-grow', grow],
    ['flex-shrink', shrink],
    ['flex-basis', basis],
  ]
}

/**
 * The `--tw-*` registers still referenced after resolution, i.e. the ones
 * with no `@property` default and no assignment in this rule.
 *
 * Only Tailwind's own registers count. A `var(--color-something)` left over
 * is a theme lookup that genuinely failed and must still be reported --
 * treating that as "inert" would quietly excuse a resolution bug.
 */
function unfilledRegisters(value: string): string[] {
  return [...value.matchAll(/var\((--tw-[a-z0-9-]+)/g)].map((m) => m[1])
}

export function normalize(block: string, vars: Map<string, string>): Normalized {
  const declarations = new Map<string, string>()
  const unresolved: string[] = []

  const parsed = splitDeclarations(block)
  // Tailwind assigns its `--tw-*` registers in the same rule that goes on
  // to reference them (`--tw-blur: blur(8px); filter: var(--tw-blur,) ...`).
  // Those assignments aren't output to compare on their own, but they have
  // to be in scope before anything else in the rule is resolved.
  const scoped = new Map(vars)
  for (const [prop, value] of parsed) {
    if (prop.startsWith('--')) scoped.set(prop, value)
  }

  for (const [prop, rawValue] of parsed) {
    if (prop.startsWith('--')) continue

    let value = resolveVars(rawValue, scoped)
    value = evaluateCalc(value)
    value = canonicalizeValue(value)

    if (value.includes('var(') || value.includes('calc(')) {
      // A `--tw-*` register with no `@property` default that nothing in
      // this rule sets is not "we couldn't work it out" -- it is a slot
      // another utility fills. `bg-conic` is
      // `conic-gradient(var(--tw-gradient-stops))` and the stops only
      // exist once a `from-*` is written beside it, so standalone the
      // declaration is invalid at computed-value time and the browser
      // drops it. Reporting that as inert is what lets the comparison
      // call it composition-only rather than leaving no claim at all.
      if (unfilledRegisters(value).length > 0) continue
      unresolved.push(`${prop}: ${rawValue}`)
      continue
    }
    for (const [expandedProp, expandedValue] of expandShorthand(prop, value)) {
      const name = canonicalizePropertyName(expandedProp)
      declarations.set(name, canonicalizeProperty(name, canonicalizeValue(expandedValue)))
    }
  }

  applyLineHeightRatio(declarations)
  return { declarations, unresolved }
}

/**
 * Tailwind states a text size's line-height as a unitless ratio of its own
 * font size; Dowel resolves it to px. With the font size present in the
 * same rule the two are directly comparable, so fold the ratio here.
 */
function applyLineHeightRatio(declarations: Map<string, string>): void {
  const lineHeight = declarations.get('line-height')
  const fontSize = declarations.get('font-size')
  if (!lineHeight || !fontSize) return
  if (/px|%|em/.test(lineHeight)) return
  const ratio = parseFloat(lineHeight)
  const size = parseFloat(fontSize)
  if (!Number.isFinite(ratio) || !Number.isFinite(size)) return
  declarations.set('line-height', canonicalizeValue(`${ratio * size}px`))
}
