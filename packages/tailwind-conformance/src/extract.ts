// Pulls `selector { declarations }` pairs out of CSS text.
//
// A brace-matching scan rather than a regex, because Tailwind nests
// at-rules *inside* style rules:
//
//   .bg-linear-to-r {
//     --tw-gradient-position: to right;
//     @supports (background-image: linear-gradient(in lab, red, red)) {
//       --tw-gradient-position: to right in oklab;
//     }
//     background-image: linear-gradient(var(--tw-gradient-stops));
//   }
//
// The regex this replaced required a rule's body to be brace-free, so a
// rule shaped like that matched nothing at all. That is not a rule the
// report got wrong -- it is a rule the report never saw, and a candidate
// the oracle has no CSS for silently leaves the denominator as "Tailwind
// emits nothing for this one". Every gradient direction utility sat
// outside the measurement for exactly that reason.
//
// A nested `@supports` block's declarations are folded into the rule
// containing them, in source order, so the enhanced value wins -- which
// is what Tailwind intends: the outer declaration is the fallback for
// engines without the feature, and the browsers this compiles for have
// it.
//
// Only `@supports`. A nested `@media` is an environment condition rather
// than a capability, and folding one in describes an element that isn't
// the element being compared: `outline-hidden` carries a
// `@media (forced-colors: active)` branch, and counting it made Dowel
// look like it was missing two declarations that only exist when the
// user has turned forced colours on.

export interface Rule {
  selector: string
  declarations: string
  /**
   * The at-rules enclosing this one, outermost first.
   *
   * Recorded because nothing compared them until 2026-08-17: the report
   * matches declaration text, so a rule that lost its `@media` wrapper --
   * or never had one -- read as identical to one that kept it. See
   * `variants.ts`.
   */
  atRules: string[]
}

export function extractRules(css: string): Rule[] {
  const rules: Rule[] = []
  // One frame per open block. `target` is the rule its declarations
  // belong to -- itself for a style rule, the enclosing rule for a
  // nested `@supports`, and nothing for anything else: a top-level
  // at-rule's body is other rules rather than declarations, and a nested
  // conditional's declarations describe a different element state.
  const stack: { target: Rule | null; opened: boolean }[] = []
  // The at-rule preludes currently open, outermost first. A nested
  // `@supports` folded into its rule is *not* one of these -- it describes
  // the same element, which is why its declarations were folded in the
  // first place.
  const open: string[] = []
  let buffer = ''

  const flush = () => {
    const target = stack.length > 0 ? stack[stack.length - 1].target : null
    if (target) target.declarations += buffer
    buffer = ''
  }

  for (let index = 0; index < css.length; index += 1) {
    const ch = css[index]

    // A brace inside a string is text, not structure. `content-['{']` is
    // rare and entirely legal.
    if (ch === '"' || ch === "'") {
      const end = closingQuote(css, index)
      buffer += css.slice(index, end + 1)
      index = end
      continue
    }

    if (ch === '{') {
      const name = buffer.trim()
      buffer = ''
      const parent = stack.length > 0 ? stack[stack.length - 1].target : null
      if (name.startsWith('@')) {
        const folded = name.startsWith('@supports')
        if (!folded) open.push(name)
        stack.push({ target: folded ? parent : null, opened: !folded })
      } else {
        const rule: Rule = { selector: name, declarations: '', atRules: [...open] }
        rules.push(rule)
        stack.push({ target: rule, opened: false })
      }
      continue
    }

    if (ch === '}') {
      // A last declaration without its semicolon still counts.
      flush()
      if (stack.pop()?.opened) open.pop()
      continue
    }

    buffer += ch
    if (ch === ';') flush()
  }

  // `@property --x { ... }` bodies are descriptors rather than
  // declarations to compare, and the `to`/`50%` steps inside `@keyframes`
  // are not rules any candidate produced. Both are filtered by the
  // callers, which know which selectors they care about.
  return rules
}

/** The index of the quote closing the one at `start`. */
function closingQuote(css: string, start: number): number {
  const quote = css[start]
  for (let i = start + 1; i < css.length; i += 1) {
    if (css[i] === '\\') {
      i += 1
      continue
    }
    if (css[i] === quote) return i
  }
  return css.length - 1
}
