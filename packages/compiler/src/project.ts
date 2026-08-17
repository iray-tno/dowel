// Project-wide source discovery and candidate-cache reconciliation, shared
// by the Vite and Metro integrations.

import { mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { performance } from 'node:perf_hooks'
import { globbySync } from 'globby'

import { openCandidateCache, type CandidateCache } from './index.ts'

const SCANNABLE = new Set(['.tsx', '.jsx', '.ts', '.js', '.mts', '.mjs'])
const DEFAULT_INCLUDE = ['**/*.{tsx,jsx,ts,js,mts,mjs}']
const DEFAULT_EXCLUDE = [
  '**/node_modules/**',
  '**/.git/**',
  '**/dist/**',
  '**/build/**',
  '**/coverage/**',
  '**/.next/**',
  '**/.nuxt/**',
  '**/.output/**',
  '**/.turbo/**',
  '**/.expo/**',
  '**/target/**',
  '**/temp/**',
  '**/tmp/**',
  '**/vendor/**',
  '**/.generated/**',
]

export const CACHE_DIR = path.join('node_modules', '.dowel')

export interface ContentOptions {
  /** Globs relative to the project root. */
  include?: string[]
  /** Additional ignore globs relative to the project root. */
  exclude?: string[]
  /** Read nested .gitignore files while walking. Defaults to true. */
  respectGitignore?: boolean
}

export interface ScanStats {
  discoveredFiles: number
  scannedFiles: number
  skippedFiles: number
  deletedFiles: number
  sourceBytes: number
  durationMs: number
}

export interface ProjectCache {
  cache: CandidateCache
  /** Absolute path of node_modules/.dowel, already created. */
  dir: string
  /** Whether the project-wide candidate set changed. */
  changed: boolean
  /** Absolute files admitted by this walk, for bundler watch filtering. */
  files: string[]
  stats: ScanStats
}

/**
 * Returns authored source files in stable order. Globby supplies gitignore
 * semantics and avoids following directory symlinks, preventing pnpm links
 * and temporary checkouts from expanding one project walk into another.
 */
export function discoverSources(root: string, options: ContentOptions = {}): string[] {
  return globbySync(options.include ?? DEFAULT_INCLUDE, {
    cwd: root,
    absolute: true,
    onlyFiles: true,
    unique: true,
    followSymbolicLinks: false,
    gitignore: options.respectGitignore ?? true,
    ignore: [...DEFAULT_EXCLUDE, ...(options.exclude ?? [])],
  })
    .filter((file) => SCANNABLE.has(path.extname(file)))
    .map((file) => path.resolve(file))
    .sort()
}

/** The real file behind a bundler module id, if Dowel should inspect it. */
export function scannableFile(id: string): string | undefined {
  if (id.startsWith('\0') || id.includes('node_modules')) return undefined
  const file = id.split('?')[0]
  return SCANNABLE.has(path.extname(file)) ? file : undefined
}

/**
 * Opens the persistent cache, scans changed sources, and removes entries for
 * files absent from this complete walk. Unchanged files are statted but never
 * read, which keeps warm starts proportional to directory traversal.
 */
export function scanProject(root: string, options: ContentOptions = {}): ProjectCache {
  const started = performance.now()
  const dir = path.join(root, CACHE_DIR)
  mkdirSync(dir, { recursive: true })
  const cache = openCandidateCache(path.join(dir, 'candidates.json'))
  const files = discoverSources(root, options)
  let scannedFiles = 0
  let skippedFiles = 0
  let sourceBytes = 0
  let changed = false

  for (const file of files) {
    const stat = statSync(file)
    if (cache.isCurrent(file, stat.mtimeMs)) {
      skippedFiles++
      continue
    }
    const source = readFileSync(file, 'utf8')
    sourceBytes += Buffer.byteLength(source)
    scannedFiles++
    changed = cache.scanFile(file, source, stat.mtimeMs) || changed
  }

  const deletedFiles = cache.retainFiles(files)
  changed = deletedFiles > 0 || changed
  cache.persist()

  return {
    cache,
    dir,
    changed,
    files,
    stats: {
      discoveredFiles: files.length,
      scannedFiles,
      skippedFiles,
      deletedFiles,
      sourceBytes,
      durationMs: performance.now() - started,
    },
  }
}

/** Writes a generated artifact only when its bytes actually changed. */
export function writeFileIfChanged(file: string, content: string): boolean {
  try {
    if (readFileSync(file, 'utf8') === content) return false
  } catch {
    // A missing or briefly unreadable generated file should be replaced.
  }
  writeFileSync(file, content)
  return true
}

export function importSpecifier(fromFile: string, target: string): string {
  const relative = path.relative(path.dirname(fromFile), target).replaceAll('\\', '/')
  return relative.startsWith('.') ? relative : `./${relative}`
}
