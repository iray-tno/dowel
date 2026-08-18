// The actual source-rewrite logic, kept separate from `index.ts`'s Metro
// wiring so it's unit-testable in plain Node without needing a running
// Metro/Expo instance (none is available to verify against in this
// environment -- no device/simulator).
//
// Mirrors @hozo/vite's approach (splice compiled JSX at the exact
// span hozo_napi reports, strip the now-unreferenced @hozo/core import)
// with two Native-specific differences:
// - View/Text/Pressable aren't JSX intrinsics on Native the way div/span/
//   button are on Web -- they're real react-native exports, so the
//   stripped @hozo/core import must be replaced with one from
//   'react-native', not just deleted.
// - Styles aren't a separate CSS file/import -- they're inlined as a
//   `const styles = StyleSheet.create({...})` declaration in the same
//   file, since that's the idiomatic RN pattern.

import { compileNative, type CompiledNativeComponent, type Theme } from '@hozo/compiler'
import { reportDiagnostics } from '@hozo/compiler/diagnostics'
import { importSpecifier } from '@hozo/compiler/project'
import { candidateModulePath } from './project.ts'

const HOZO_CORE_IMPORT_RE = /import\s*\{[^}]*\}\s*from\s*['"]@hozo\/core['"]\s*\n?/
/// The components the Native backend lowers to that come from
/// `react-native` itself. `HozoSpaced` and `HozoDialog` are Hozo's own
/// and arrive through `runtimeImports` instead.
///
/// `TextInput` was missing here until 2026-08-16, so a compiled TextInput
/// referred to an identifier nothing imported. Metro bundles that happily
/// -- an undefined identifier is only an error when it runs -- so the
/// example built cleanly and would have crashed on first render. Reading
/// the bundle is what found it; building it was not enough.
const RN_PRIMITIVE_TAGS = ['View', 'Text', 'Pressable', 'TextInput', 'Image', 'ScrollView', 'FlatList', 'RefreshControl'] as const
const RN_VALUE_EXPORTS = ['PanResponder'] as const

/// Renames this component's `hozoN`/`hozoN_suffix` style/JSX identifiers
/// to be unique across every component in the file -- each `compileNative`
/// call starts counting from `hozo0` independently per root, so two
/// components in the same source file would otherwise collide when their
/// styles are merged into one `StyleSheet.create({...})`.
function namespaceHozoIdentifiers(text: string, rootIndex: number): string {
  return text.replace(/\bhozo(\d+)/g, `hozo_r${rootIndex}_$1`)
}

function mergeStyleObjects(blocks: string[]): string {
  if (blocks.length === 1) {
    return blocks[0]
  }
  const inner = blocks.map((block) => block.trim().replace(/^\{/, '').replace(/\}$/, '').trim()).join('\n')
  return `{\n${inner}\n}`
}

/**
 * Returns the rewritten source, or `null` if there's nothing for Hozo to
 * do (not a `.tsx` file, or no `@hozo/core` usage found).
 */
export function transformHozoSource(
  code: string,
  filename: string,
  projectRoot?: string,
  theme?: Theme,
): string | null {
  if (!filename.endsWith('.tsx') || !code.includes('@hozo/core')) {
    return null
  }

  const components = compileNative(code, theme)
  if (components.length === 0) {
    return null
  }

  // Error-severity diagnostics stop the build. The case this exists for --
  // a Web-only utility like `block`/`grid` reaching the Native backend --
  // has no correct Native output, so continuing would ship a layout that
  // looks right on Web and is silently wrong on device. The policy itself
  // is shared with every other integration now; see
  // `@hozo/compiler/diagnostics` for why it stopped being Metro's alone.
  reportDiagnostics(
    components.flatMap((component) => component.diagnostics),
    filename,
    // eslint-disable-next-line no-console
    (message) => console.warn(message),
  )

  const usedTags = new Set<string>()
  const styleBlocks: string[] = []
  components.forEach((component: CompiledNativeComponent, index: number) => {
    styleBlocks.push(namespaceHozoIdentifiers(component.styles, index))
    for (const tag of RN_PRIMITIVE_TAGS) {
      if (new RegExp(`<${tag}[\\s/>]`).test(component.jsx)) {
        usedTags.add(tag)
      }
    }
    for (const name of RN_VALUE_EXPORTS) {
      if (new RegExp(`\\b${name}\\b`).test(code)) usedTags.add(name)
    }
    // `View`/`Text`/`Pressable` carried through `Child::Verbatim` are fine:
    // they resolve to the react-native imports above, which are the very
    // components Hozo lowers to. `Button` is not -- Hozo's Button is a
    // semantic primitive that lowers to Pressable, while react-native's
    // takes a `title` prop and renders no children. Neither that nor
    // `@hozo/core`'s Web `<button>` fallback works on a device, so this is
    // refused rather than silently mis-rendered.
    if (/<Button[\s/>]/.test(component.jsx)) {
      throw new Error(
        `[hozo] ${filename}: a <Button> is inside an expression the compiler can't read, so it ` +
          `can't be lowered -- and React Native's own Button is a different component with a ` +
          `different API. Move it out of the expression, or use Pressable directly.`,
      )
    }
  })

  // Every rewrite as an offset-keyed edit, applied back-to-front so
  // earlier offsets stay valid. Two kinds share the list: replacing a
  // component's JSX, and inserting its hook declarations at the top of the
  // enclosing function.
  const edits: { start: number; end: number; text: string }[] = []
  const runtimeImports = new Set<string>()
  // Two components can live in one function, and both may need the same
  // hook. The binding is function-scoped, so a second `const` would be a
  // redeclaration -- and a second call would change the hook order.
  const declaredPerSlot = new Map<number, Set<string>>()

  components.forEach((component: CompiledNativeComponent, index: number) => {
    edits.push({
      start: component.spanStart,
      end: component.spanEnd,
      text: namespaceHozoIdentifiers(component.jsx, index),
    })

    // Collected before the prelude check, not inside it. Runtime imports
    // used to come only from hooks, which always have a prelude, so the two
    // were folded together -- and then `HozoSpaced` and `HozoDialog`
    // arrived, which need an import and no hook. Every component using
    // `space-*`, `divide-*` or a `Dialog` was emitting a module that
    // referenced an undefined identifier, which nothing caught because
    // nothing ran the output.
    for (const name of component.runtimeImports) {
      runtimeImports.add(name)
    }

    if (component.prelude.length === 0) {
      return
    }
    if (component.hookSlot === null || component.hookSlot === undefined) {
      // A hook needs a statement position. There isn't one at module
      // scope or in a concise arrow body, and inlining the call into the
      // JSX would break the rules of hooks the moment the element sits
      // behind a conditional.
      throw new Error(
        `[hozo] ${filename}: \`dark:\` and breakpoint variants need a React hook, which can ` +
          `only go inside a component function. Move this JSX into a function component with a ` +
          `block body (\`function C() { return <View .../> }\`).`,
      )
    }

    const already = declaredPerSlot.get(component.hookSlot) ?? new Set<string>()
    const fresh = component.prelude.filter((line) => !already.has(line))
    for (const line of fresh) {
      already.add(line)
    }
    declaredPerSlot.set(component.hookSlot, already)
    if (fresh.length > 0) {
      edits.push({
        start: component.hookSlot,
        end: component.hookSlot,
        text: `\n  ${fresh.join('\n  ')}`,
      })
    }
  })

  let next = code
  for (const edit of [...edits].sort((a, b) => b.start - a.start)) {
    next = next.slice(0, edit.start) + edit.text + next.slice(edit.end)
  }

  next = next.replace(HOZO_CORE_IMPORT_RE, '')

  const rnImports = [...usedTags, 'StyleSheet'].join(', ')
  const mergedStyles = mergeStyleObjects(styleBlocks)
  next = `import { ${rnImports} } from 'react-native'\nconst styles = StyleSheet.create(${mergedStyles})\n${next}`
  if (runtimeImports.size > 0) {
    next = `import { ${[...runtimeImports].join(', ')} } from '@hozo/runtime'\n${next}`
  }

  // Only when something actually calls it. The candidate module is
  // generated at config time (see `./project.ts`); a file with no
  // unresolvable className must not depend on it having been generated.
  if (next.includes('hozoClasses(')) {
    if (projectRoot === undefined) {
      throw new Error(
        `[hozo] ${filename} has a className the compiler can't read, which needs the generated ` +
          `candidate module -- but no projectRoot was given, so its location is unknown. Call ` +
          `generateCandidateModule(projectRoot) from metro.config.js.`,
      )
    }
    const specifier = importSpecifier(filename, candidateModulePath(projectRoot))
    next = `import { hozoClasses } from '${specifier}'\n${next}`
  }

  return next
}
