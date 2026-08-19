import { existsSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { writeFileIfChanged, type HozoProjectOptions } from '@hozo/compiler/project'

import { generateCandidateModule } from './project.ts'

/**
 * The same options every Hozo integration takes, under this one's name.
 *
 * `root` was spelled `projectRoot` here until the four integrations were
 * lined up against each other. Metro's own config key keeps that name and
 * is still the default; the Hozo option that overrides it is `root`, the
 * same word Vite and Next use.
 */
export type HozoMetroOptions = HozoProjectOptions

interface MetroConfigShape {
  projectRoot?: string
  transformer?: Record<string, unknown> & { babelTransformerPath?: string }
  [key: string]: unknown
}

/**
 * What the config layer has to tell the transformer.
 *
 * They are separate processes -- Metro transforms in `jest-worker`
 * subprocesses -- and the only thing they reliably share is the project
 * root, so anything configured in `metro.config.js` reaches the transform
 * through this file.
 */
export interface HozoMetroState {
  upstreamTransformer?: string
  css?: string
  sources?: readonly string[]
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
  const projectRoot = path.resolve(options.root ?? config.projectRoot ?? process.cwd())
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
    `${JSON.stringify(
      { upstreamTransformer, css: options.css, sources: options.sources } satisfies HozoMetroState,
      null,
      2,
    )}\n`,
  )

  return {
    ...config,
    transformer: {
      ...config.transformer,
      babelTransformerPath: transformerPath,
    },
  }
}
