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

import { existsSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'
import type { Plugin, ViteDevServer } from 'vite'
import { compile, type CandidateCache, type Theme } from '@dowel/compiler'
import { loadTheme } from '@dowel/tailwind'
import {
  discoverSources,
  importSpecifier,
  scanProject,
  scannableFile,
  writeFileIfChanged,
  type ContentOptions,
} from '@dowel/compiler/project'

const DOWEL_CORE_IMPORT_RE = /import\s*\{[^}]*\}\s*from\s*['"]@dowel\/core['"]\s*\n?/

/// The names `@dowel/core` exports. A lowered element never mentions these
/// (it becomes `div`/`span`/`button`), so one surviving in the output came
/// through `Child::Verbatim` -- something the compiler carried rather than
/// understood.
const DOWEL_PRIMITIVES = ['View', 'Text', 'Pressable', 'Button', 'Link', 'TextInput', 'Dialog', 'Image', 'ScrollView'] as const

/**
 * Whether any Dowel primitive name is still mentioned after lowering.
 *
 * This decides whether the `@dowel/core` import may be removed. Stripping
 * it unconditionally -- which this plugin did until 2026-08-15 -- was safe
 * only while unmodeled children were being *deleted*. Now that they're
 * carried, anything the compiler couldn't lower in place survives to the
 * output, and the import is what makes it resolve.
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
 */
function referencesDowelPrimitive(code: string): boolean {
  return DOWEL_PRIMITIVES.some((name) => new RegExp(`\\b${name}\\b`).test(code))
}

/// Renames this component's `dowel-N` class names to be unique across every
/// component in the file -- `compile()` starts counting from `dowel-0`
/// independently per root, so two components in the same source file would
/// otherwise collide once their CSS is merged into one companion file.
/// `dowel-view` (no digits) is the intentionally-shared base class and
/// must NOT be touched by this.
function namespaceDowelClasses(text: string, rootIndex: number): string {
  return text.replace(/\bdowel-(\d+)\b/g, `dowel-r${rootIndex}-$1`)
}


/**
 * A project's design tokens reach the compiler from here.
 *
 * Without them Dowel resolves against Tailwind's defaults, which is right
 * until a project defines its own -- and then `bg-brand` compiles to a CSS
 * variable nothing defines and `p-4` to the wrong number of pixels. The
 * theme is read once at `buildStart` rather than per file: it is a
 * project-wide fact, and re-reading it for every module would run
 * Tailwind's resolver hundreds of times for one answer.
 */
export interface DowelOptions {
  /**
   * The stylesheet that carries `@import "tailwindcss"` and the project's
   * `@theme`. Relative to the Vite root.
   *
   * Left out, Dowel looks for the usual names and falls back to the
   * default theme if it finds none -- reporting what it looked for rather
   * than silently compiling against the wrong palette.
   */
  css?: string
  /** Source globs and ignores used by the project-wide dynamic-class scan. */
  content?: ContentOptions
  /** Report project-scan work and timing through Vite's logger. */
  debug?: boolean
}

/// Where a Tailwind v4 project usually keeps its entry stylesheet. Only
/// consulted when `css` isn't given.
const CSS_GUESSES = ['src/index.css', 'src/styles.css', 'src/app.css', 'app/globals.css']

export function dowel(options: DowelOptions = {}): Plugin {
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
    name: 'dowel',
    enforce: 'pre',

    configResolved(config) {
      root = config.root
    },

    configureServer(devServer) {
      server = devServer
    },

    async buildStart() {
      theme = await readProjectTheme(root, options.css, (message) => this.warn(message))

      // The whole project, not just what the bundler happens to reach: a
      // class can be produced by a module the graph never resolves
      // statically.
      const project = scanProject(root, options.content)
      cache = project.cache
      includedFiles = new Set(project.files)
      candidateCssPath = path.join(project.dir, 'candidates.css')
      writeCandidateCss()
      if (options.debug) {
        const s = project.stats
        this.info(
          `[dowel] discovered ${s.discoveredFiles} files; scanned ${s.scannedFiles}, ` +
            `skipped ${s.skippedFiles}, removed ${s.deletedFiles} in ${s.durationMs.toFixed(1)}ms`,
        )
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
      if (file && includedFiles.has(path.resolve(file))) {
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

      const components = compile(code, theme)
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

      // Only when nothing needs it. A primitive that survived lowering
      // (carried through `Child::Verbatim`) still has to resolve, and
      // `@dowel/core` exports real working React components for exactly
      // this -- proposal §2.3's "fall back gracefully". Such an element
      // renders with its raw class string instead of a compiled scoped
      // class, which the project-wide candidate stylesheet may well cover.
      // Degraded, not broken.
      if (!referencesDowelPrimitive(next)) {
        next = next.replace(DOWEL_CORE_IMPORT_RE, '')
      }

      const cssFileName = `${path.basename(id)}.dowel.css`
      const cssPath = path.join(path.dirname(id), cssFileName)
      writeFileIfChanged(cssPath, css)
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

/**
 * Loads the project's theme, or nothing if there is none to load.
 *
 * Nothing means Tailwind's defaults, which is exactly what a project
 * without a `@theme` wants -- so a missing stylesheet is reported and then
 * shrugged off rather than being an error. A *named* stylesheet that
 * doesn't exist is different: someone said where it was, and silently
 * compiling against the wrong palette would be worse than a warning.
 */
async function readProjectTheme(
  root: string,
  configured: string | undefined,
  warn: (message: string) => void,
): Promise<Theme | undefined> {
  const candidates = configured ? [configured] : CSS_GUESSES
  for (const relative of candidates) {
    const file = path.resolve(root, relative)
    if (!existsSync(file)) continue
    try {
      return await loadTheme(readFileSync(file, 'utf8'), path.dirname(file))
    } catch (error) {
      warn(
        `[dowel] couldn't read the theme from ${relative}, so utilities resolve against ` +
          `Tailwind's defaults: ${(error as Error).message}`,
      )
      return undefined
    }
  }
  if (configured) {
    warn(`[dowel] no stylesheet at ${configured}, so utilities resolve against Tailwind's defaults`)
  }
  return undefined
}
