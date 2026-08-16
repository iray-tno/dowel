// Finding and loading the project's design tokens.
//
// Shared by the two places that need them: the config-time candidate
// module, and the transformer itself. The transformer runs in a
// `jest-worker` subprocess and shares nothing with the config, so it reads
// the theme itself rather than being handed one -- cached per process,
// since Metro transforms hundreds of files and Tailwind's resolver is not
// cheap enough to run per file.

import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'

import type { Theme } from '@dowel/compiler'
import { loadTheme } from '@dowel/tailwind'

/// Where a Tailwind v4 React Native project usually keeps its entry
/// stylesheet. Only consulted when no path is given.
const CSS_GUESSES = ['global.css', 'src/global.css', 'app/global.css', 'src/index.css']

const cache = new Map<string, Theme | undefined>()

/**
 * The project's theme, or `undefined` for Tailwind's defaults.
 *
 * Undefined is a real answer, not a failure: a project without a
 * `@theme` wants exactly the defaults. What would be a failure is
 * compiling half the app against one palette and half against another,
 * which is why this is cached per project rather than per call.
 */
export async function readProjectTheme(
  projectRoot: string,
  configured?: string,
): Promise<Theme | undefined> {
  const key = `${projectRoot}\u0000${configured ?? ''}`
  if (cache.has(key)) {
    return cache.get(key)
  }
  const theme = await load(projectRoot, configured)
  cache.set(key, theme)
  return theme
}

async function load(projectRoot: string, configured?: string): Promise<Theme | undefined> {
  for (const relative of configured ? [configured] : CSS_GUESSES) {
    const file = path.resolve(projectRoot, relative)
    if (!existsSync(file)) continue
    try {
      return await loadTheme(readFileSync(file, 'utf8'), path.dirname(file))
    } catch (error) {
      // Reported rather than thrown: the defaults are a usable answer, and
      // stopping a build over a theme that won't parse would be a worse
      // trade than compiling with a palette the message names.
      // eslint-disable-next-line no-console
      console.warn(
        `[dowel] couldn't read the theme from ${relative}, so utilities resolve against ` +
          `Tailwind's defaults: ${(error as Error).message}`,
      )
      return undefined
    }
  }
  return undefined
}
