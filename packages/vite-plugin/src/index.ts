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
//
// Alongside that, one project-wide stylesheet covers the classes the
// compiler *couldn't* read (proposal §7's third tier). Those come from a
// byte scan of every source file rather than from the AST, so this plugin
// owns the project walk and the file-deletion signal, while the cache in
// Rust owns scanning, staleness, and persistence.

import { statSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import type { Plugin, ViteDevServer } from 'vite'
import { compile, type CandidateCache } from '@dowel/compiler'
import { importSpecifier, scanProject, scannableFile } from '@dowel/compiler/project'

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
  let root = process.cwd()
  let cache: CandidateCache
  let candidateCssPath = ''
  let server: ViteDevServer | undefined

  /// Regenerates the project-wide candidate stylesheet and, in dev, makes
  /// the already-loaded module pick it up. The file lives under
  /// `node_modules`, which Vite's watcher ignores by default, so the
  /// invalidation has to be explicit rather than left to the watcher.
  function writeCandidateCss() {
    writeFileSync(candidateCssPath, cache.renderCss())
    const module = server?.moduleGraph.getModuleById(candidateCssPath)
    if (module) {
      void server?.reloadModule(module)
    }
  }

  return {
    name: 'dowel',
    enforce: 'pre',

    configResolved(config) {
      root = config.root
    },

    configureServer(devServer) {
      server = devServer
    },

    buildStart() {
      // The whole project, not just what the bundler happens to reach: a
      // class can be produced by a module the graph never resolves
      // statically.
      const project = scanProject(root)
      cache = project.cache
      candidateCssPath = path.join(project.dir, 'candidates.css')
      writeCandidateCss()
    },

    watchChange(id, change) {
      // Without this a deleted file's classes would stay in the stylesheet
      // for as long as the cache file survives, since nothing else ever
      // revisits an entry that stopped being scanned.
      if (change.event === 'delete' && cache?.forget(path.resolve(id))) {
        writeCandidateCss()
      }
    },

    transform(code, id) {
      const file = scannableFile(id)
      if (file) {
        // `enforce: 'pre'` means `code` is still the source as written,
        // which is what the scanner expects. Keyed by the same absolute
        // path `scanProject`'s walk used, so a file scanned there isn't
        // recorded twice under two spellings.
        const modifiedMs = statSync(file, { throwIfNoEntry: false })?.mtimeMs ?? 0
        if (cache.scanFile(path.resolve(file), code, modifiedMs)) {
          writeCandidateCss()
        }
      }

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
      // Imported from every lowered file rather than from one designated
      // entry: the candidate sheet has to be present whichever module the
      // dynamic className lives in, and Vite resolves the repeated import
      // to a single module in the graph.
      next = `import './${cssFileName}'\nimport '${importSpecifier(id, candidateCssPath)}'\n${next}`

      return { code: next, map: null }
    },

    buildEnd() {
      cache?.persist()
    },
  }
}
