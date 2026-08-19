// Web bundler integration (proposal §A: Vite before Metro). Splices
// hozo_web's compiled JSX directly into the original source at the exact
// span the Rust side reported, rather than replacing the whole file --
// that's what lets a component keep its own hooks/handlers/other logic
// untouched while only its View/Text/Pressable/Button usage gets lowered.
//
// CSS is written to a real `<file>.hozo.css` companion file next to the
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

import { statSync } from 'node:fs'
import path from 'node:path'
import type { Plugin, ViteDevServer } from 'vite'
import type { CandidateCache, Theme } from '@hozo/compiler'
import { loadProjectTheme } from '@hozo/tailwind'
import { reportDiagnostics } from '@hozo/compiler/diagnostics'
import { lowerModule, sideEffectImport } from '@hozo/compiler/lower'
import {
  discoverSources,
  importSpecifier,
  scanProject,
  scanSummary,
  scannableFile,
  writeFileIfChanged,
  type HozoProjectOptions,
} from '@hozo/compiler/project'

/**
 * The same options every Hozo integration takes, under this one's name.
 *
 * Nothing is added, deliberately. The design tokens a project defines
 * reach the compiler through `css`, and without them Hozo resolves against
 * Tailwind's defaults -- right until a project defines its own, and then
 * `bg-brand` compiles to a CSS variable nothing defines and `p-4` to the
 * wrong number of pixels. The theme is read once at `buildStart` rather
 * than per file: it is a project-wide fact, and re-reading it for every
 * module would run Tailwind's resolver hundreds of times for one answer.
 */
export type HozoOptions = HozoProjectOptions

export function hozo(options: HozoOptions = {}): Plugin {
  let theme: Theme | undefined
  let root = process.cwd()
  let cache: CandidateCache
  let candidateCssPath = ''
  let includedFiles = new Set<string>()
  let server: ViteDevServer | undefined

  /// Regenerates the project-wide candidate stylesheet and, in dev, makes
  /// the already-loaded module pick it up. The file lives under
  /// `node_modules`, which Vite's watcher ignores by default, so the
  /// invalidation has to be explicit rather than left to the watcher.
  function writeCandidateCss() {
    if (!writeFileIfChanged(candidateCssPath, cache.renderCss(theme))) return false
    const module = server?.moduleGraph.getModuleById(candidateCssPath)
    if (module) {
      void server?.reloadModule(module)
    }
    return true
  }

  return {
    name: 'hozo',
    enforce: 'pre',

    configResolved(config) {
      root = options.root ?? config.root
    },

    configureServer(devServer) {
      server = devServer
    },

    async buildStart() {
      theme = await loadProjectTheme(root, {
        css: options.css,
        warn: (message) => this.warn(message),
      })

      // The whole project, not just what the bundler happens to reach: a
      // class can be produced by a module the graph never resolves
      // statically.
      const project = scanProject(root, options.content)
      cache = project.cache
      includedFiles = new Set(project.files)
      candidateCssPath = path.join(project.dir, 'candidates.css')
      writeCandidateCss()
      if (options.debug) {
        this.info(scanSummary(project.stats))
      }
    },

    watchChange(id, change) {
      // Without this a deleted file's classes would stay in the stylesheet
      // for as long as the cache file survives, since nothing else ever
      // revisits an entry that stopped being scanned.
      if (change.event === 'delete') {
        const absolute = path.resolve(id)
        includedFiles.delete(absolute)
        if (cache?.forget(absolute)) writeCandidateCss()
      }
      if (change.event === 'create') {
        const absolute = path.resolve(id)
        const relative = path.relative(root, absolute).replaceAll('\\', '/')
        if (discoverSources(root, { ...options.content, include: [relative] }).includes(absolute)) {
          includedFiles.add(absolute)
        }
      }
    },

    transform(code, id) {
      const file = scannableFile(id)
      const isDerivedModule = id.includes('?')
      if (file && !isDerivedModule && includedFiles.has(path.resolve(file))) {
        // `enforce: 'pre'` means `code` is still the source as written,
        // which is what the scanner expects. Keyed by the same absolute
        // path `scanProject`'s walk used, so a file scanned there isn't
        // recorded twice under two spellings.
        const modifiedMs = statSync(file, { throwIfNoEntry: false })?.mtimeMs ?? 0
        if (cache.scanFile(path.resolve(file), code, modifiedMs)) {
          writeCandidateCss()
        }
      }

      if (!file) return
      const lowered = lowerModule(code, id, file, theme, { sources: options.sources })
      if (!lowered) return

      // Shared with Metro and Next, which is new: this warned on
      // everything including error-severity diagnostics, and Metro threw.
      // The difference looked deliberate and wasn't -- every error Hozo
      // can emit today comes from the Native backend, so this had simply
      // never been handed one.
      reportDiagnostics(lowered.diagnostics, file, (message) => this.warn(message))
      // Declined rather than compiled: the file keeps its own primitives
      // and must not gain an import for a stylesheet that was never
      // written.
      if (!lowered.lowered) return

      let next = lowered.code
      writeFileIfChanged(lowered.cssPath, lowered.css)
      // Imported from every lowered file rather than from one designated
      // entry: the candidate sheet has to be present whichever module the
      // dynamic className lives in, and Vite resolves the repeated import
      // to a single module in the graph.
      // Generate the declarations through JSON.stringify: Storybook's
      // extensionless-import checker inspects preset dependency source and
      // otherwise mistakes our code-generating string for a real import.
      next =
        sideEffectImport(`./${lowered.cssFileName}`) +
        sideEffectImport(importSpecifier(file, candidateCssPath)) +
        next

      return { code: next, map: null }
    },

    buildEnd() {
      cache?.persist()
    },
  }
}
