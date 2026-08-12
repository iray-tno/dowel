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

export interface CompiledNativeComponent {
  jsx: string
  styles: string
  diagnostics: CompileDiagnostic[]
  spanStart: number
  spanEnd: number
}

interface NativeBinding {
  compile(source: string): CompiledComponent[]
  compileNative(source: string): CompiledNativeComponent[]
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

// Not yet wired into a Metro transformer (@dowel/vite-plugin's Metro
// counterpart doesn't exist yet -- Native was deliberately validated after
// Web, per the A-phase decision). Exposed now so the binding layer mirrors
// both backends; the transformer-side integration is separate future work.
export function compileNative(source: string): CompiledNativeComponent[] {
  return loadNative().compileNative(source)
}
