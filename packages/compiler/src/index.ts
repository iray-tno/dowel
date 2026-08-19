// Dev-only loader: requires the native addon copied next to this file by
// `scripts/build-native.mjs` (`pnpm build:native`). Native `.node` addons
// load via CJS `require`, even from an ESM package -- hence `createRequire`
// rather than a dynamic `import()`. See that script's header comment for
// why this isn't @napi-rs/cli-packaged yet.

import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'

import { loadNativeBinding } from './native-loader.ts'

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
  /// Named imports `prelude` needs from `@hozo/runtime`.
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
  /// The Native equivalent: a JS module exporting `hozoClasses`, a
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
    native = loadNativeBinding<NativeBinding>({
      require,
      localPath: fileURLToPath(new URL('../hozo_napi.node', import.meta.url)),
    })
  }
  return native
}

/**
 * A project's design tokens, as `@hozo/tailwind` extracts them.
 *
 * Optional everywhere: an absent theme means Tailwind's default palette,
 * which is what every caller got before themes existed. Passing one only
 * ever resolves more, never less.
 */
export interface Theme {
  colors: { token: string; oklch: string; hex: string }[]
}

export function compile(
  source: string,
  theme?: Theme,
  sources?: readonly string[],
): CompiledComponent[] {
  // `sources` is per *tag*: a name imported from a module not on the list
  // is carried verbatim instead of lowered. Left out, every module is
  // trusted, which is what a caller with no project configuration wants.
  return loadNative().compile(source, theme, sources ? [...sources] : undefined)
}

// Not yet wired into a Metro transformer (@hozo/vite's Metro
// counterpart doesn't exist yet -- Native was deliberately validated after
// Web, per the A-phase decision). Exposed now so the binding layer mirrors
// both backends; the transformer-side integration is separate future work.
export function compileNative(
  source: string,
  theme?: Theme,
  sources?: readonly string[],
): CompiledNativeComponent[] {
  return loadNative().compileNative(source, theme, sources ? [...sources] : undefined)
}

export function openCandidateCache(path?: string): CandidateCache {
  return new (loadNative().CandidateCache)(path)
}

/** One imported binding whose local name is a Hozo primitive tag. */
export interface PrimitiveImport {
  local: string
  module: string
}

/**
 * Where a file's primitive-named bindings come from.
 *
 * The compiler matches on the JSX tag name and never asks where the name
 * came from. That is what lets a plain React Native file compile without
 * changing a line of it -- and equally what would let a `<View>` from some
 * other component library be lowered to a `<div>`. See
 * `@hozo/compiler/sources` for the policy built on this.
 */
export function primitiveImports(source: string): PrimitiveImport[] {
  return loadNative().primitiveImports(source)
}

/**
 * Every binding a source file imports from one module, by local name.
 *
 * The Native backend prepends its own `react-native` import, and a React
 * Native file already has one -- re-declaring a name it already binds is a
 * SyntaxError rather than a harmless duplicate.
 */
export function moduleImports(source: string, module: string): string[] {
  return loadNative().moduleImports(source, module)
}
