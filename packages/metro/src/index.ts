// Metro's custom-transformer contract: a module whose `transform` export
// receives `{ src, filename, options, ... }` and returns whatever the
// wrapped/upstream transformer returns. We only rewrite `src` before
// handing off -- everything else (JSX-to-JS compilation, the rest of
// Babel's pipeline) stays the upstream transformer's job, same division
// of labor as @hozo/vite running `enforce: 'pre'` ahead of
// @vitejs/plugin-react.
//
// NOT verified against a running Metro/Expo build -- no device/simulator
// available in this environment. `transformHozoSource` (the part that
// actually matters) is covered by unit tests in `transform.test.ts`
// instead; this file is thin, documented Metro API surface on top of it.

import { createRequire } from 'node:module'
import path from 'node:path'
import { transformHozoSource } from './transform.ts'
import { readProjectTheme } from './theme.ts'

const require = createRequire(import.meta.url)

interface TransformParams {
  src: string
  filename: string
  /// Metro's transform options. `projectRoot` is what locates the
  /// generated candidate module -- this transformer runs in a `jest-worker`
  /// subprocess, so it shares nothing else with the config that wrote it.
  options: { projectRoot?: string } & Record<string, unknown>
  [key: string]: unknown
}

interface UpstreamTransformer {
  transform(params: TransformParams): unknown
}

let upstream: UpstreamTransformer | undefined

/// The transformer this one wraps, in the order a project is likely to
/// have it.
///
/// `metro-react-native-babel-transformer` was the only name here until
/// 2026-08-16, and React Native renamed it at 0.73 -- so on any currently
/// supported version this package required something that isn't installed,
/// and the bundle died inside React Native's own source with a syntax
/// error that named neither. Found by building the example, which is the
/// only thing that runs this file at all.
///
/// `HOZO_UPSTREAM_TRANSFORMER` overrides the search, for projects (Expo
/// among them) that ship their own.
const UPSTREAM_CANDIDATES = [
  '@react-native/metro-babel-transformer',
  '@expo/metro-config/babel-transformer',
  'metro-react-native-babel-transformer',
]

function loadUpstream(projectRoot?: string): UpstreamTransformer {
  if (upstream) {
    return upstream
  }
  const configured = process.env.HOZO_UPSTREAM_TRANSFORMER
  const candidates = configured ? [configured] : UPSTREAM_CANDIDATES
  // Resolved from the *project*, not from this package. The upstream
  // transformer is the consuming app's dependency, and under pnpm's strict
  // layout a package cannot see its consumer's -- so resolving relative to
  // this file finds nothing in exactly the setup a monorepo has.
  const fromProject = projectRoot
    ? createRequire(path.join(projectRoot, 'noop.js'))
    : require
  const tried: string[] = []
  for (const name of candidates) {
    for (const resolve of [fromProject, require]) {
      try {
        upstream = resolve(name) as UpstreamTransformer
        return upstream
      } catch {
        // Next resolver, then next candidate.
      }
    }
    tried.push(name)
  }
  throw new Error(
    `[hozo] no Babel transformer for Metro to hand off to. Hozo only rewrites the source and ` +
      `leaves the rest of the pipeline alone, so it needs the one your project already uses. ` +
      `Tried: ${tried.join(', ')}. Set HOZO_UPSTREAM_TRANSFORMER to the right one.`,
  )
}

// Async because the theme comes from Tailwind's own resolver, which is
// async. Metro allows it, and the alternative -- compiling against the
// default palette while the project defines its own -- is the failure this
// exists to prevent.
export async function transform(params: TransformParams): Promise<unknown> {
  const projectRoot = params.options?.projectRoot
  const theme = projectRoot ? await readProjectTheme(projectRoot) : undefined
  const rewritten = transformHozoSource(params.src, params.filename, projectRoot, theme)
  const nextParams = rewritten === null ? params : { ...params, src: rewritten }
  return loadUpstream(projectRoot).transform(nextParams)
}

export { transformHozoSource } from './transform.ts'
export { generateCandidateModule, candidateModulePath } from './project.ts'
