// The part of "run a source file through Hozo" that has nothing to do with
// any one bundler.
//
// It lived inside `@hozo/vite`'s `transform` until Next.js needed the same
// hundred lines. What is actually bundler-specific turns out to be small --
// where diagnostics go, when files are written, how a companion stylesheet
// is referenced -- and everything else is the same work: compile the
// module, splice each component back at the span the Rust side reported,
// namespace the class names so two components in one file can't collide,
// and decide whether the `@hozo/core` import may go.
//
// Splitting it here rather than having Next import the Vite plugin keeps
// the dependency honest: a Next.js project has no Vite in it.

import path from 'node:path'

import { compile, type CompileDiagnostic, type Theme } from './index.ts'

const HOZO_CORE_IMPORT_RE = /import\s*\{[^}]*\}\s*from\s*['"]@hozo\/core['"]\s*\n?/

/**
 * The names `@hozo/core` exports. A lowered element never mentions these
 * (it becomes `div`/`span`/`button`), so one surviving in the output came
 * through `Child::Verbatim` -- something the compiler carried rather than
 * understood.
 */
const HOZO_PRIMITIVES = [
  'View',
  'Text',
  'Paragraph',
  'Heading',
  'Section',
  'Article',
  'Nav',
  'List',
  'ListItem',
  'Pressable',
  'Button',
  'Link',
  'TextInput',
  'Dialog',
  'Image',
  'ScrollView',
  'FlatList',
  'PanResponder',
] as const

/**
 * Whether any Hozo primitive name is still mentioned after lowering.
 *
 * This decides whether the `@hozo/core` import may be removed. Stripping it
 * unconditionally was safe only while unmodeled children were being
 * *deleted*. Now that they're carried, anything the compiler couldn't lower
 * in place survives to the output, and the import is what makes it resolve.
 *
 * Deliberately a word match rather than a `<Tag` match, and deliberately
 * biased toward keeping the import. A primitive can be referenced without
 * ever appearing as a tag (`const Label = Text` then `<Label/>`), and the
 * two failure modes are not symmetric: an unnecessary import is dead weight
 * a bundler drops, while a missing one breaks at runtime. On Web it doesn't
 * even break cleanly -- `Text` is a DOM global (the text-node interface),
 * so React is handed a DOM class where a component belongs and throws
 * something unrelated to the cause. `View` at least gives an honest
 * ReferenceError.
 *
 * The bias goes all the way: the import statement is part of the text this
 * searches, so in practice it always finds a primitive and the import
 * always survives. What actually removes it is the bundler's own
 * unused-specifier elision, once lowering has left nothing referring to it
 * -- which is the same outcome by a route that cannot be wrong. This
 * remains as the check that would keep a *used* one, and it is the reason
 * `PanResponder.create(...)` beside a lowered tree still resolves.
 */
export function referencesHozoPrimitive(code: string): boolean {
  return HOZO_PRIMITIVES.some((name) => new RegExp(`\\b${name}\\b`).test(code))
}

/**
 * Renames this component's `hozo-N` class names to be unique across every
 * component in the file.
 *
 * `compile()` starts counting from `hozo-0` independently per root, so two
 * components in the same source file would otherwise collide once their CSS
 * is merged into one companion file. `hozo-view` (no digits) is the
 * intentionally-shared base class and must NOT be touched by this.
 */
export function namespaceHozoClasses(text: string, rootIndex: number): string {
  return text.replace(/\bhozo-(\d+)\b/g, `hozo-r${rootIndex}-$1`)
}

/**
 * A stable short hash of a module id.
 *
 * Route-splitting frameworks transform several query-qualified modules from
 * one source file, and each derived module owns different JSX -- so sharing
 * one companion stylesheet path would make the last transform overwrite the
 * others' CSS.
 */
export function moduleIdHash(id: string): string {
  let hash = 0x811c9dc5
  for (let index = 0; index < id.length; index++) {
    hash = Math.imul(hash ^ id.charCodeAt(index), 0x01000193)
  }
  return (hash >>> 0).toString(36)
}

export function sideEffectImport(specifier: string): string {
  return `import ${JSON.stringify(specifier)}\n`
}

export interface LoweredModule {
  /** The source with every compiled component spliced back in. */
  code: string
  /** Every component's CSS, concatenated in source order. */
  css: string
  /** The companion stylesheet's file name, unique per derived module. */
  cssFileName: string
  /** Its absolute path, next to the source file. */
  cssPath: string
  diagnostics: CompileDiagnostic[]
}

/**
 * Lowers one module, or `undefined` when there is nothing to lower.
 *
 * `undefined` rather than an unchanged result on purpose: a caller that
 * returns the source it was given still marks the module as transformed,
 * and the distinction matters to Next's loader chain.
 */
export function lowerModule(
  code: string,
  id: string,
  file: string,
  theme: Theme | undefined,
): LoweredModule | undefined {
  if (!file.endsWith('.tsx') || !code.includes('@hozo/core')) return undefined

  const components = compile(code, theme)
  if (components.length === 0) return undefined

  let next = code
  let css = ''
  // Splice from the last span to the first so earlier offsets stay valid as
  // later (in the string, not necessarily in array order) edits are applied.
  const bySpanDescending = components
    .map((component, index) => ({ component, index }))
    .sort((a, b) => b.component.spanStart - a.component.spanStart)
  for (const { component, index } of bySpanDescending) {
    const jsx = namespaceHozoClasses(component.jsx, index)
    const componentCss = namespaceHozoClasses(component.css, index)
    next = next.slice(0, component.spanStart) + jsx + next.slice(component.spanEnd)
    css = componentCss + css
  }

  // Only when nothing needs it. A primitive that survived lowering (carried
  // through `Child::Verbatim`) still has to resolve, and `@hozo/core`
  // exports real working React components for exactly this -- proposal
  // §2.3's "fall back gracefully".
  if (!referencesHozoPrimitive(next)) {
    next = next.replace(HOZO_CORE_IMPORT_RE, '')
  }

  const isDerivedModule = id.includes('?')
  const cssFileName = isDerivedModule
    ? `${path.basename(file)}.${moduleIdHash(id)}.hozo.css`
    : `${path.basename(file)}.hozo.css`

  return {
    code: next,
    css,
    cssFileName,
    cssPath: path.join(path.dirname(file), cssFileName),
    diagnostics: components.flatMap((component) => component.diagnostics),
  }
}
