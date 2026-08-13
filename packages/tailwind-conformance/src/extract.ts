// Pulls `selector { declarations }` pairs out of CSS text.
//
// Both sides wrap conditional utilities in at-rules (`@media (width >=
// 48rem) { .md\:flex-row { ... } }`), so a naive `\{([^}]*)\}` scan latches
// onto the at-rule's opening brace and captures the inner selector as if it
// were a declaration. Requiring the body to be brace-free makes the match
// skip the wrapper and land on the real rule instead.

export interface Rule {
  selector: string
  declarations: string
}

export function extractRules(css: string): Rule[] {
  const rules: Rule[] = []
  const re = /([^{}]+)\{([^{}]*)\}/g
  let match: RegExpExecArray | null
  while ((match = re.exec(css))) {
    const selector = match[1].trim()
    // Skip at-rule prelude-only matches (`@property --x { ... }` bodies are
    // descriptors, not declarations to compare).
    if (selector.startsWith('@')) continue
    rules.push({ selector, declarations: match[2] })
  }
  return rules
}
