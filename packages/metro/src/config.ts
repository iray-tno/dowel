import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { writeFileIfChanged, type ContentOptions } from '@hozo/compiler/project'

import { generateCandidateModule } from './project.ts'

export interface HozoMetroOptions {
  /** Metro project root. Defaults to config.projectRoot, then process.cwd(). */
  projectRoot?: string
  /** Tailwind entry stylesheet, relative to projectRoot. */
  css?: string
  /** Source globs and ignores used by the project-wide candidate scan. */
  content?: ContentOptions
}

interface MetroConfigShape {
  projectRoot?: string
  transformer?: Record<string, unknown> & { babelTransformerPath?: string }
  [key: string]: unknown
}

export interface HozoMetroState {
  upstreamTransformer?: string
  css?: string
}

export const METRO_STATE_FILE = 'metro.json'

export function metroStatePath(projectRoot: string): string {
  return path.join(projectRoot, 'node_modules', '.hozo', METRO_STATE_FILE)
}

export function readMetroState(projectRoot: string): HozoMetroState | undefined {
  try {
    return JSON.parse(readFileSync(metroStatePath(projectRoot), 'utf8')) as HozoMetroState
  } catch {
    return undefined
  }
}

function currentTransformerPath(): string {
  const directory = path.dirname(fileURLToPath(import.meta.url))
  const built = path.join(directory, 'index.js')
  return existsSync(built) ? built : path.join(directory, 'index.ts')
}

/**
 * Adds Hozo to an existing Metro configuration without discarding the
 * transformer's other settings. The previous Babel transformer is recorded
 * as Hozo's upstream, so Expo and custom transformer chains keep working.
 */
export async function withHozo<T extends MetroConfigShape>(
  configOrPromise: T | Promise<T>,
  options: HozoMetroOptions = {},
): Promise<T> {
  const config = await configOrPromise
  const projectRoot = path.resolve(options.projectRoot ?? config.projectRoot ?? process.cwd())
  const transformerPath = currentTransformerPath()
  const configuredUpstream = config.transformer?.babelTransformerPath
  const upstreamTransformer =
    configuredUpstream && path.resolve(configuredUpstream) !== path.resolve(transformerPath)
      ? configuredUpstream
      : undefined

  await generateCandidateModule(projectRoot, {
    css: options.css,
    content: options.content,
  })
  writeFileIfChanged(
    metroStatePath(projectRoot),
    `${JSON.stringify({ upstreamTransformer, css: options.css } satisfies HozoMetroState, null, 2)}\n`,
  )

  return {
    ...config,
    transformer: {
      ...config.transformer,
      babelTransformerPath: transformerPath,
    },
  }
}
