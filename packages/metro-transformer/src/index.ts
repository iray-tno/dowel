// Metro's custom-transformer contract: a module whose `transform` export
// receives `{ src, filename, options, ... }` and returns whatever the
// wrapped/upstream transformer returns. We only rewrite `src` before
// handing off -- everything else (JSX-to-JS compilation, the rest of
// Babel's pipeline) stays the upstream transformer's job, same division
// of labor as @dowel/vite-plugin running `enforce: 'pre'` ahead of
// @vitejs/plugin-react.
//
// NOT verified against a running Metro/Expo build -- no device/simulator
// available in this environment. `transformDowelSource` (the part that
// actually matters) is covered by unit tests in `transform.test.ts`
// instead; this file is thin, documented Metro API surface on top of it.

import { createRequire } from 'node:module'
import { transformDowelSource } from './transform.ts'

const require = createRequire(import.meta.url)

interface TransformParams {
  src: string
  filename: string
  options: unknown
  [key: string]: unknown
}

interface UpstreamTransformer {
  transform(params: TransformParams): unknown
}

let upstream: UpstreamTransformer | undefined

function loadUpstream(): UpstreamTransformer {
  if (!upstream) {
    // Peer dependency: whatever transformer the consuming RN/Expo project
    // already uses (bare RN CLI projects default to this package name;
    // Expo projects configure their own path -- see this package's
    // README for how to point at that instead).
    upstream = require('metro-react-native-babel-transformer') as UpstreamTransformer
  }
  return upstream
}

export function transform(params: TransformParams): unknown {
  const rewritten = transformDowelSource(params.src, params.filename)
  const nextParams = rewritten === null ? params : { ...params, src: rewritten }
  return loadUpstream().transform(nextParams)
}

export { transformDowelSource } from './transform.ts'
