// Next.js integration.
//
// The first non-Vite bundler Hozo runs under, and the shape of the problem
// is genuinely different rather than differently spelled. A Vite plugin has
// lifecycle hooks -- `buildStart` to read the theme and walk the project,
// `watchChange` for deletions, `buildEnd` to persist. Next.js gives a
// config object and a loader, and under Turbopack there is no plugin API at
// all: a loader is the only place user code runs.
//
// So the project-wide half happens here, while `next.config.ts` is being
// evaluated. That is once per build, before anything is compiled, in the
// Next process itself -- which is exactly when a Vite plugin's `buildStart`
// would run. The per-file half is the loader, and it re-scans each module
// it is handed so a class written during `next dev` reaches the candidate
// stylesheet without a restart.

import { createRequire } from 'node:module'
import path from 'node:path'

import {
  scanProject,
  scanSummary,
  writeFileIfChanged,
  type HozoProjectOptions,
} from '@hozo/compiler/project'

/**
 * The same options every Hozo integration takes, under this one's name.
 *
 * `root` defaults to the working directory, which is what Next.js sets it
 * to while it evaluates the config.
 */
export type HozoNextOptions = HozoProjectOptions

/** What the loader needs, resolved once so every worker gets the same answer. */
export interface HozoLoaderOptions extends HozoNextOptions {
  root: string
  candidateCssPath: string
}

/**
 * A Next.js config with Hozo installed.
 *
 * Registers the same loader twice, because Next has two bundlers and no
 * shared way to reach both: `turbopack.rules` for the default one and
 * `webpack()` for `next build --webpack`. The loader itself is identical --
 * Turbopack implements the webpack loader interface Hozo needs -- so this
 * is registration, not two implementations.
 */
export function withHozo<T extends Record<string, unknown>>(
  nextConfig: T = {} as T,
  options: HozoNextOptions = {},
): T {
  const root = options.root ?? process.cwd()
  const loaderOptions = prepareProject(root, options)
  // Resolved to an absolute path rather than left as a bare specifier.
  // Turbopack resolves loader names against the *project* root, and in a
  // workspace this package is a symlink under a `node_modules/.pnpm` path
  // that a bare name doesn't reach.
  const loader = createRequire(import.meta.url).resolve('../loader.js')

  const rule = {
    loaders: [{ loader, options: loaderOptions }],
    as: '*.tsx',
  }

  return {
    ...nextConfig,
    turbopack: {
      ...(nextConfig.turbopack as Record<string, unknown> | undefined),
      rules: {
        ...((nextConfig.turbopack as { rules?: Record<string, unknown> } | undefined)?.rules ?? {}),
        '*.tsx': rule,
      },
    },
    webpack(config: WebpackConfig, context: unknown) {
      config.module ??= { rules: [] }
      config.module.rules ??= []
      // `enforce: 'pre'`, the same declaration the Vite plugin makes and
      // for the same reason: Hozo has to see the source as written. Array
      // position is not enough, and it reads backwards -- webpack runs a
      // module's loaders right-to-left, so an earlier rule runs *later*.
      // Without this the SWC transform got there first, Hozo was handed
      // compiled JavaScript, found no JSX to lower, and left the
      // `@hozo/core` import in place; the App Router then refused the build
      // because that module calls `useState` in a server component.
      config.module.rules.unshift({
        enforce: 'pre',
        test: /\.tsx$/,
        exclude: /node_modules/,
        use: [{ loader, options: loaderOptions }],
      })
      const user = (nextConfig as { webpack?: (c: WebpackConfig, x: unknown) => WebpackConfig })
        .webpack
      return user ? user(config, context) : config
    },
  } as unknown as T
}

interface WebpackConfig {
  module?: { rules?: unknown[] }
}

/**
 * Walks the project and writes the candidate stylesheet, returning what the
 * loader needs to keep it current.
 *
 * Synchronous and eager, unlike the Vite plugin's `buildStart`. There is
 * nowhere later to put it: Turbopack has no build-start hook, and a
 * stylesheet written after the first module was compiled would be a
 * stylesheet the first module didn't import.
 */
function prepareProject(root: string, options: HozoNextOptions): HozoLoaderOptions {
  const project = scanProject(root, options.content)
  const candidateCssPath = path.join(project.dir, 'candidates.css')
  // Rendered with no theme: the theme is loaded asynchronously and this
  // runs synchronously. The loader rewrites the file with the real theme
  // before any module that imports it is compiled, and an empty file here
  // is what makes the import resolve at all.
  writeFileIfChanged(candidateCssPath, project.cache.renderCss(undefined))
  project.cache.persist()
  if (options.debug) {
    console.info(scanSummary(project.stats))
  }
  return { ...options, root, candidateCssPath }
}
