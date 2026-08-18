// Metro's half of the candidate scan (proposal §7's third tier on Native).
//
// Why this is a separate config-time step, where Vite does it inside the
// plugin: Metro runs `transform` in `jest-worker` subprocesses. Several
// workers transform files concurrently, so scanning and writing from there
// would mean several writers to one cache file. Metro's *config* layer is
// ordinary main-process code, which is where this runs -- one writer, no
// locking needed, exactly the ownership the cache crate assumes.
//
// KNOWN LIMITATION: the candidate module is generated once, at config load.
// A class that only becomes a candidate after Metro started -- a new string
// literal in a helper module -- won't be in the map until Metro is
// restarted. `react-native-css` documents the same restriction for its own
// transformer. The generated module is deliberately written under
// `node_modules/.hozo/`, so a restart with a cleared cache regenerates it.

import path from 'node:path'

import {
  scanProject,
  scanSummary,
  writeFileIfChanged,
  type HozoProjectOptions,
} from '@hozo/compiler/project'
import { loadProjectTheme } from '@hozo/tailwind'

/// File name of the generated resolver module. Also read by the
/// transformer, which imports it into every file it lowers.
export const CANDIDATE_MODULE = 'candidates.native.js'

/**
 * Absolute path of the generated resolver module for `projectRoot`.
 *
 * Derived rather than passed around because the transformer runs in a
 * separate process from the config that generated it -- the only thing
 * they reliably share is the project root, which Metro gives the
 * transformer in its options.
 */
export function candidateModulePath(projectRoot: string): string {
  return path.join(projectRoot, 'node_modules', '.hozo', CANDIDATE_MODULE)
}

/**
 * Scans the project and writes the candidate resolver module. Call from
 * `metro.config.js` before returning the config.
 *
 * Returns the module's path, mostly so a caller can log it.
 */
/**
 * Generates the project-wide candidate module, resolving against the
 * project's theme.
 *
 * Async because reading the theme means asking Tailwind, and Tailwind's
 * design-system loader is async. Metro config files can await, and the
 * alternative -- resolving these classes against the default palette
 * while every other class in the app uses the project's -- would be two
 * different answers for the same utility in one bundle.
 */
export async function generateCandidateModule(
  projectRoot: string,
  options: HozoProjectOptions = {},
): Promise<string> {
  const theme = await loadProjectTheme(projectRoot, {
    css: options.css,
    warn: (message) => console.warn(message),
  })
  const { cache, stats } = scanProject(projectRoot, options.content)
  if (options.debug) {
    // eslint-disable-next-line no-console
    console.info(scanSummary(stats))
  }
  const modulePath = candidateModulePath(projectRoot)
  writeFileIfChanged(modulePath, cache.renderNativeModule(theme))
  return modulePath
}
