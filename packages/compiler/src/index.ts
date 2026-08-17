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
  /// Statements to splice at `hookSlot` for `jsx` to work. Empty unless a
  /// condition needed a React hook (`dark:`, breakpoints).
  prelude: string[]
  /// Named imports `prelude` needs from `@dowel/runtime`.
  runtimeImports: string[]
  /// Byte offset just inside the enclosing function's `{` -- the only safe
  /// place for `prelude`, since a hook must be called unconditionally and
  /// in the same order every render. Absent (`null`/`undefined` -- napi
  /// marshals a Rust `None` as `undefined`) when this JSX isn't inside a
  /// function body a statement can go in: module scope, or a concise arrow.
  hookSlot: number | null | undefined
  diagnostics: CompileDiagnostic[]
  spanStart: number
  spanEnd: number
}

/// Accumulates the project's runtime-resolvable class candidates (proposal
/// §7's third tier) and turns them into one stylesheet. See the Rust side's
/// doc comment for why this is project-wide rather than per file.
export interface CandidateCache {
  /// True when `path` was already scanned at exactly this mtime -- the
  /// caller can skip reading the file at all.
  isCurrent(path: string, modifiedMs: number): boolean
  /// Records a scan of `source`. Returns whether the candidate set changed,
  /// so an unchanged one doesn't cause a stylesheet rewrite.
  scanFile(path: string, source: string, modifiedMs: number): boolean
  forget(path: string): boolean
  /// Drops cached contributions from files absent from a complete walk.
  retainFiles(paths: string[]): number
  /// The Web stylesheet: rules under the classes' real Tailwind names, for
  /// the browser's own CSS engine to match.
  renderCss(theme?: Theme): string
  /// The Native equivalent: a JS module exporting `dowelClasses`, a
  /// resolver bound to this project's class-name -> style-object map.
  renderNativeModule(theme?: Theme): string
  persist(): void
  readonly size: number
}

interface CandidateCacheConstructor {
  /// `path` is where the cache persists between builds; omit it to keep the
  /// cache in memory only.
  new (path?: string): CandidateCache
}

interface NativeBinding {
  compile(source: string): CompiledComponent[]
  compileNative(source: string): CompiledNativeComponent[]
  CandidateCache: CandidateCacheConstructor
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

/**
 * A project's design tokens, as `@dowel/tailwind` extracts them.
 *
 * Optional everywhere: an absent theme means Tailwind's default palette,
 * which is what every caller got before themes existed. Passing one only
 * ever resolves more, never less.
 */
export interface Theme {
  colors: { token: string; oklch: string; hex: string }[]
}

export function compile(source: string, theme?: Theme): CompiledComponent[] {
  return loadNative().compile(source, theme)
}

// Not yet wired into a Metro transformer (@dowel/vite-plugin's Metro
// counterpart doesn't exist yet -- Native was deliberately validated after
// Web, per the A-phase decision). Exposed now so the binding layer mirrors
// both backends; the transformer-side integration is separate future work.
export function compileNative(source: string, theme?: Theme): CompiledNativeComponent[] {
  return loadNative().compileNative(source, theme)
}

export function openCandidateCache(path?: string): CandidateCache {
  return new (loadNative().CandidateCache)(path)
}
