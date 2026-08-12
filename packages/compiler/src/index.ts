// Dev-only loader: requires the native addon copied next to this file by
// `scripts/build-native.mjs` (`pnpm build:native`). Native `.node` addons
// load via CJS `require`, even from an ESM package -- hence `createRequire`
// rather than a dynamic `import()`. See that script's header comment for
// why this isn't @napi-rs/cli-packaged yet.

import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)

export interface CompileDiagnostic {
  code: string
  severity: string
  message: string
  spanStart: number
  spanEnd: number
}

export interface CompiledComponent {
  jsx: string
  css: string
  diagnostics: CompileDiagnostic[]
  spanStart: number
  spanEnd: number
}

interface NativeBinding {
  compile(source: string): CompiledComponent[]
}

let native: NativeBinding | undefined

function loadNative(): NativeBinding {
  if (!native) {
    try {
      native = require('../dowel_napi.node') as NativeBinding
    } catch (cause) {
      throw new Error(
        '@dowel/compiler: native addon not found. Run `pnpm --filter @dowel/compiler build:native` first.',
        { cause },
      )
    }
  }
  return native
}

export function compile(source: string): CompiledComponent[] {
  return loadNative().compile(source)
}
