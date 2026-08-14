// The actual source-rewrite logic, kept separate from `index.ts`'s Metro
// wiring so it's unit-testable in plain Node without needing a running
// Metro/Expo instance (none is available to verify against in this
// environment -- no device/simulator).
//
// Mirrors @dowel/vite-plugin's approach (splice compiled JSX at the exact
// span dowel_napi reports, strip the now-unreferenced @dowel/core import)
// with two Native-specific differences:
// - View/Text/Pressable aren't JSX intrinsics on Native the way div/span/
//   button are on Web -- they're real react-native exports, so the
//   stripped @dowel/core import must be replaced with one from
//   'react-native', not just deleted.
// - Styles aren't a separate CSS file/import -- they're inlined as a
//   `const styles = StyleSheet.create({...})` declaration in the same
//   file, since that's the idiomatic RN pattern.

import { compileNative, type CompiledNativeComponent } from '@dowel/compiler'

const DOWEL_CORE_IMPORT_RE = /import\s*\{[^}]*\}\s*from\s*['"]@dowel\/core['"]\s*\n?/
const RN_PRIMITIVE_TAGS = ['View', 'Text', 'Pressable'] as const

/// Renames this component's `dowelN`/`dowelN_suffix` style/JSX identifiers
/// to be unique across every component in the file -- each `compileNative`
/// call starts counting from `dowel0` independently per root, so two
/// components in the same source file would otherwise collide when their
/// styles are merged into one `StyleSheet.create({...})`.
function namespaceDowelIdentifiers(text: string, rootIndex: number): string {
  return text.replace(/\bdowel(\d+)/g, `dowel_r${rootIndex}_$1`)
}

function mergeStyleObjects(blocks: string[]): string {
  if (blocks.length === 1) {
    return blocks[0]
  }
  const inner = blocks.map((block) => block.trim().replace(/^\{/, '').replace(/\}$/, '').trim()).join('\n')
  return `{\n${inner}\n}`
}

/**
 * Returns the rewritten source, or `null` if there's nothing for Dowel to
 * do (not a `.tsx` file, or no `@dowel/core` usage found).
 */
export function transformDowelSource(code: string, filename: string): string | null {
  if (!filename.endsWith('.tsx') || !code.includes('@dowel/core')) {
    return null
  }

  const components = compileNative(code)
  if (components.length === 0) {
    return null
  }

  // Error-severity diagnostics stop the build. The case this exists for --
  // a Web-only utility like `block`/`grid` reaching the Native backend --
  // has no correct Native output, so continuing would ship a layout that
  // looks right on Web and is silently wrong on device.
  const errors = components.flatMap((c) => c.diagnostics.filter((d) => d.severity === 'error'))
  if (errors.length > 0) {
    const detail = errors.map((d) => `  ${d.code}: ${d.message}`).join('\n')
    throw new Error(`[dowel] ${filename} cannot be compiled for React Native:\n${detail}`)
  }

  for (const component of components) {
    for (const diagnostic of component.diagnostics) {
      // eslint-disable-next-line no-console
      console.warn(`[dowel] ${diagnostic.code}: ${diagnostic.message}`)
    }
  }

  const usedTags = new Set<string>()
  const styleBlocks: string[] = []
  components.forEach((component: CompiledNativeComponent, index: number) => {
    styleBlocks.push(namespaceDowelIdentifiers(component.styles, index))
    for (const tag of RN_PRIMITIVE_TAGS) {
      if (component.jsx.includes(`<${tag}`)) {
        usedTags.add(tag)
      }
    }
  })

  let next = code
  // Splice from the last span to the first so earlier offsets stay valid
  // as later (in the string, not array order) edits are applied.
  const bySpanDescending = components
    .map((component: CompiledNativeComponent, index: number) => ({ component, index }))
    .sort((a, b) => b.component.spanStart - a.component.spanStart)
  for (const { component, index } of bySpanDescending) {
    const jsx = namespaceDowelIdentifiers(component.jsx, index)
    next = next.slice(0, component.spanStart) + jsx + next.slice(component.spanEnd)
  }

  next = next.replace(DOWEL_CORE_IMPORT_RE, '')

  const rnImports = [...usedTags, 'StyleSheet'].join(', ')
  const mergedStyles = mergeStyleObjects(styleBlocks)
  next = `import { ${rnImports} } from 'react-native'\nconst styles = StyleSheet.create(${mergedStyles})\n${next}`

  return next
}
