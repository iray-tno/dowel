// Web bundler integration (proposal §A: Vite before Metro). Splices
// dowel_web's compiled JSX directly into the original source at the exact
// span the Rust side reported, rather than replacing the whole file --
// that's what lets a component keep its own hooks/handlers/other logic
// untouched while only its View/Text/Pressable/Button usage gets lowered.
//
// CSS is written to a real `<file>.dowel.css` companion file next to the
// source and imported normally, rather than served through a Vite virtual
// module -- simpler and more robust for this pass than fighting Vite's
// virtual-module CSS-type detection; a real cache directory or virtual
// module is a reasonable thing to move to once this needs to survive
// production builds cleanly.

import { writeFileSync } from 'node:fs'
import path from 'node:path'
import type { Plugin } from 'vite'
import { compile } from '@dowel/compiler'

const DOWEL_CORE_IMPORT_RE = /import\s*\{[^}]*\}\s*from\s*['"]@dowel\/core['"]\s*\n?/

/// Renames this component's `dowel-N` class names to be unique across every
/// component in the file -- `compile()` starts counting from `dowel-0`
/// independently per root, so two components in the same source file would
/// otherwise collide once their CSS is merged into one companion file.
/// `dowel-view` (no digits) is the intentionally-shared base class and
/// must NOT be touched by this.
function namespaceDowelClasses(text: string, rootIndex: number): string {
  return text.replace(/\bdowel-(\d+)\b/g, `dowel-r${rootIndex}-$1`)
}

export function dowel(): Plugin {
  return {
    name: 'dowel',
    enforce: 'pre',

    transform(code, id) {
      if (!id.endsWith('.tsx') || !code.includes('@dowel/core')) {
        return
      }

      const components = compile(code)
      if (components.length === 0) {
        return
      }

      let next = code
      let css = ''
      // Splice from the last span to the first so earlier offsets stay
      // valid as later (in the string, not necessarily in array order)
      // edits are applied.
      const bySpanDescending = components
        .map((component, index) => ({ component, index }))
        .sort((a, b) => b.component.spanStart - a.component.spanStart)
      for (const { component, index } of bySpanDescending) {
        const jsx = namespaceDowelClasses(component.jsx, index)
        const componentCss = namespaceDowelClasses(component.css, index)
        next = next.slice(0, component.spanStart) + jsx + next.slice(component.spanEnd)
        css = componentCss + css
      }

      for (const component of components) {
        for (const diagnostic of component.diagnostics) {
          this.warn(`[dowel] ${diagnostic.code}: ${diagnostic.message}`)
        }
      }

      // Phase 0 scope: dowel_parser only recognizes View/Text/Pressable/
      // Button, and every recognized usage above just got fully lowered,
      // so nothing from '@dowel/core' is referenced in `next` anymore.
      next = next.replace(DOWEL_CORE_IMPORT_RE, '')

      const cssFileName = `${path.basename(id)}.dowel.css`
      const cssPath = path.join(path.dirname(id), cssFileName)
      writeFileSync(cssPath, css)
      next = `import './${cssFileName}'\n${next}`

      return { code: next, map: null }
    },
  }
}
