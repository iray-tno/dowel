// The webpack/Turbopack loader.
//
// Plain `.js` rather than the `.ts` the rest of this repository is written
// in, because a loader is resolved and executed by the bundler rather than
// imported by a tool that understands TypeScript. `@hozo/storybook`'s
// `preset.js` exists for the same reason.
//
// Asynchronous because the theme is: `loadProjectTheme` reads and runs the
// project's Tailwind entry stylesheet. It caches by root, so the cost is
// paid once per process no matter how many modules pass through here.

import { statSync } from 'node:fs'
import path from 'node:path'

import { openCandidateCache } from '@hozo/compiler'
import { lowerModule, sideEffectImport } from '@hozo/compiler/lower'
import {
  CACHE_DIR,
  importSpecifier,
  scannableFile,
  writeFileIfChanged,
} from '@hozo/compiler/project'
import { loadProjectTheme } from '@hozo/tailwind'

/** One cache and one theme per worker process, keyed by project root. */
const projects = new Map()

function projectState(options) {
  let state = projects.get(options.root)
  if (!state) {
    const cache = openCandidateCache(path.join(options.root, CACHE_DIR, 'candidates.json'))
    const theme = loadProjectTheme(options.root, {
      css: options.css,
      warn: (message) => console.warn(`[hozo] ${message}`),
    })
    state = {
      cache,
      // Rewritten once the theme resolves, unconditionally.
      //
      // `withHozo` writes this file synchronously while `next.config.ts`
      // is evaluated -- it has to exist before the first module imports
      // it -- and a theme cannot be read synchronously, so that first
      // write has none. The per-file rescan below then only rewrites when
      // the candidate set *changed*, which on a warm cache is never. So
      // the theme-less version survived, and a project token in a dynamic
      // className compiled to `var(--hozo-color-brand)` with nothing
      // defining it: no error, no warning, just no colour. Found by
      // reading what `next dev` actually served.
      theme: theme.then((resolved) => {
        writeFileIfChanged(options.candidateCssPath, cache.renderCss(resolved))
        return resolved
      }),
    }
    projects.set(options.root, state)
  }
  return state
}

export default function hozoLoader(source) {
  const callback = this.async()
  const options = this.getOptions() ?? {}
  const file = scannableFile(this.resourcePath)
  if (!file) {
    callback(null, source)
    return
  }
  const state = projectState(options)

  state.theme.then(
    (theme) => {
      try {
        // Every module that reaches this loader is rescanned, which is how
        // a class only a runtime expression produces stays covered while
        // `next dev` is running. The cache decides whether anything
        // actually changed.
        const modifiedMs = statSync(file, { throwIfNoEntry: false })?.mtimeMs ?? 0
        if (state.cache.scanFile(path.resolve(file), source, modifiedMs)) {
          writeFileIfChanged(options.candidateCssPath, state.cache.renderCss(theme))
          state.cache.persist()
        }

        const lowered = lowerModule(source, this.resourcePath, file, theme, {
          sources: options.sources,
        })
        if (!lowered) {
          callback(null, source)
          return
        }
        for (const diagnostic of lowered.diagnostics) {
          this.emitWarning(hozoWarning(diagnostic))
        }
        writeFileIfChanged(lowered.cssPath, lowered.css)
        // Both stylesheets are imported from the module itself rather than
        // from one designated entry: the candidate sheet has to be present
        // whichever module the dynamic className lives in, and a bundler
        // resolves the repeated import to a single module in its graph.
        const code =
          sideEffectImport(`./${lowered.cssFileName}`) +
          sideEffectImport(importSpecifier(file, options.candidateCssPath)) +
          lowered.code
        callback(null, code)
      } catch (error) {
        callback(error)
      }
    },
    (error) => callback(error),
  )
}

/**
 * A Hozo diagnostic as the two bundlers each want it.
 *
 * An `Error` because webpack says so -- hand it a string and it prints
 * `(Emitted value instead of an instance of Error)` in front of the
 * message. Turbopack accepts either and wraps a string in an Error itself.
 *
 * With the stack replaced by the message, because both then print it: the
 * frames point into this loader, which is where the warning was *raised*
 * and never where it is *about*. Both bundlers already attribute the
 * warning to the right source module on their own.
 */
function hozoWarning(diagnostic) {
  const warning = new Error(`[hozo] ${diagnostic.code}: ${diagnostic.message}`)
  warning.name = 'HozoDiagnostic'
  warning.stack = warning.message
  return warning
}
