// The project walk behind the candidate scan, shared by both bundler
// integrations.
//
// It lives here rather than in either plugin because both need exactly the
// same thing and for the same reason: a class can be produced by a module
// the bundler's own graph never resolves statically, so the set has to come
// from walking the source tree rather than from the graph.

import { mkdirSync, readdirSync, readFileSync, statSync } from 'node:fs'
import path from 'node:path'

import { openCandidateCache, type CandidateCache } from './index.ts'

/// Files worth scanning. Not just `.tsx`: a class string can live in any
/// module the app pulls in -- a `variants.ts` map, a plain helper -- and be
/// handed to `className` from there.
const SCANNABLE = new Set(['.tsx', '.jsx', '.ts', '.js', '.mts', '.mjs'])

/// Directories never worth walking. `node_modules` in particular would turn
/// a scan of a small app into a scan of its entire dependency tree.
const SKIP_DIRS = new Set(['node_modules', '.git', 'dist', 'build', '.next', 'coverage'])

/// Where the cache and generated files live. Under `node_modules` by
/// convention -- already git-ignored, already understood as derived -- and
/// deliberately outside the source tree, since none of it is authored.
export const CACHE_DIR = path.join('node_modules', '.dowel')

/**
 * The real file behind a module id, or `undefined` if there isn't one.
 *
 * Not every id a bundler hands a transform is a path: virtual modules are
 * `\0`-prefixed and dev requests carry `?v=` query strings. Both have to be
 * filtered out before anything touches the filesystem.
 */
export function scannableFile(id: string): string | undefined {
  if (id.startsWith('\0') || id.includes('node_modules')) {
    return undefined
  }
  const file = id.split('?')[0]
  return SCANNABLE.has(path.extname(file)) ? file : undefined
}

function* walkSources(dir: string): Generator<string> {
  let entries
  try {
    entries = readdirSync(dir, { withFileTypes: true })
  } catch {
    // A directory that vanished mid-walk or that we can't read isn't worth
    // failing a build over -- at worst its classes miss the stylesheet.
    return
  }
  for (const entry of entries) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      if (!SKIP_DIRS.has(entry.name) && !entry.name.startsWith('.')) {
        yield* walkSources(full)
      }
    } else if (SCANNABLE.has(path.extname(entry.name))) {
      yield full
    }
  }
}

export interface ProjectCache {
  cache: CandidateCache
  /// Absolute path of `node_modules/.dowel`, already created.
  dir: string
}

/**
 * Opens the project's candidate cache and brings it up to date by scanning
 * every source file that changed since the last build. Files still current
 * are skipped without being read.
 */
export function scanProject(root: string): ProjectCache {
  const dir = path.join(root, CACHE_DIR)
  mkdirSync(dir, { recursive: true })
  const cache = openCandidateCache(path.join(dir, 'candidates.json'))

  for (const file of walkSources(root)) {
    const modifiedMs = statSync(file).mtimeMs
    if (cache.isCurrent(file, modifiedMs)) {
      continue
    }
    cache.scanFile(file, readFileSync(file, 'utf8'), modifiedMs)
  }
  cache.persist()

  return { cache, dir }
}

/**
 * Import specifier for `target` as seen from `fromFile`, in the
 * forward-slash form module specifiers require even on Windows.
 */
export function importSpecifier(fromFile: string, target: string): string {
  const relative = path.relative(path.dirname(fromFile), target).replaceAll('\\', '/')
  return relative.startsWith('.') ? relative : `./${relative}`
}
