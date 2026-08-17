// Runs the Native backend's output.
//
// What this establishes, and what it doesn't, is worth being exact about.
// React Native ships Flow-typed JavaScript that Node can't parse, so the
// real `react-native` is not importable here and this renders against a
// stub. So it does **not** prove that React Native's runtime accepts the
// output -- only a device or a simulator does that.
//
// What it does prove is the part nothing else touches: that the generated
// module parses and evaluates, that the tree it builds has the styles and
// props on the components they were meant for, and that Dowel's own runtime
// components behave -- `DowelSpaced` distributing a parent's `space-*` over
// children, `DowelDialog` opening. Those are ordinary React, and until now
// they had never run.
//
// It pairs with the type check, which asks React Native's own declarations
// whether each emitted style is one it would accept. Types say the styles
// are valid; this says the tree is assembled correctly.

import { execFileSync } from 'node:child_process'
import { registerHooks } from 'node:module'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { transformDowelSource } from '@dowel/metro-transformer'

const require = createRequire(import.meta.url)

// Redirects every `react-native` import in the graph -- the generated
// module's, and `@dowel/runtime`'s and `@dowel/a11y`'s own -- to the stub.
// A `require` shim isn't enough: those packages are ESM and Node's loader
// resolves their imports before any of this runs.
// `act` warns without it, and the warning is noise rather than signal
// here: this is a renderer, not a test of concurrent behaviour.
;(globalThis as Record<string, unknown>).IS_REACT_ACT_ENVIRONMENT = true

registerHooks({
  resolve(specifier, context, next) {
    if (specifier === 'react-native') {
      return { url: new URL('./react-native-stub.js', import.meta.url).href, shortCircuit: true }
    }
    // Metro picks a `.native` entry ahead of the plain one; Node does not.
    // This has to be a resolve hook rather than a `require` shim, because
    // these packages import each other -- `@dowel/runtime` re-exports
    // `DowelDialog` from `@dowel/a11y` -- and those imports are resolved by
    // Node's loader, out of reach of anything the generated module is
    // handed. Without it the Web dialog loads and renders a `<dialog>`.
    if (specifier === '@dowel/runtime' || specifier === '@dowel/a11y') {
      const root = path.dirname(require.resolve(`${specifier}/package.json`))
      return {
        url: pathToFileURL(path.join(root, 'src', 'index.native.ts')).href,
        shortCircuit: true,
      }
    }
    return next(specifier, context)
  },
  // Node strips types from `.ts` but has no JSX transform, and Dowel's
  // runtime components are `.tsx` -- shipped as source, because a bundler
  // is what consumes them. Transpiling them here keeps that source
  // idiomatic instead of writing `createElement` by hand to suit a test.
  load(url, context, next) {
    if (!url.endsWith('.tsx')) return next(url, context)
    return {
      format: 'module',
      shortCircuit: true,
      source: transpileTsx(fileURLToPath(url)),
    }
  },
})

function transpileTsx(file: string): string {
  const dir = mkdtempSync(path.join(packageRoot(), '.tsx-'))
  try {
    const copy = path.join(dir, 'module.tsx')
    writeFileSync(copy, readFileSync(file, 'utf8'))
    execFileSync(
      process.execPath,
      [tscBinary(), copy, '--jsx', 'react-jsx', '--module', 'esnext', '--target', 'es2022',
       '--outDir', dir, '--types', '', '--noCheck'],
      { encoding: 'utf8', stdio: 'pipe' },
    )
    return readFileSync(path.join(dir, 'module.js'), 'utf8')
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

function tscBinary(): string {
  return path.join(path.dirname(require.resolve('typescript/package.json')), 'bin', 'tsc')
}

function packageRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
}

/** A rendered tree, as `react-test-renderer` reports it. */
export type Tree = {
  type: string
  props: Record<string, unknown>
  children: (Tree | string)[] | null
} | null

export interface NativeLayoutBox {
  width: number
  height: number
}

/**
 * Transforms a source file the way Metro would, runs it, and renders the
 * named export.
 *
 * Uses `transformDowelSource` rather than assembling the module here, so
 * what runs is what ships -- including the `StyleSheet.create` wrapper and
 * the `@dowel/runtime` imports, which are where a mistake would otherwise
 * hide behind a hand-written approximation.
 */
export function renderNative(
  source: string,
  componentName: string,
  scope: Record<string, unknown> = {},
): Tree {
  return renderNativeFixture(source, componentName, scope, [])
}

/**
 * Renders a generated Native component and drives every host `onLayout`
 * callback in tree order. This is not a device substitute, but it executes
 * the same React state loop used by measurement-dependent runtime helpers.
 * Each pass must name every currently mounted layout target so tree changes
 * cannot accidentally make a fixture assert against the wrong element.
 */
export function renderNativeWithLayouts(
  source: string,
  componentName: string,
  layoutPasses: readonly (readonly NativeLayoutBox[])[],
  scope: Record<string, unknown> = {},
): Tree {
  return renderNativeFixture(source, componentName, scope, layoutPasses)
}

function renderNativeFixture(
  source: string,
  componentName: string,
  scope: Record<string, unknown>,
  layoutPasses: readonly (readonly NativeLayoutBox[])[],
): Tree {
  const transformed = transformDowelSource(source, 'Component.tsx')
  if (transformed === null) {
    throw new Error('the transformer declined this source')
  }

  const dir = mkdtempSync(path.join(packageRoot(), '.native-render-'))
  try {
    writeFileSync(path.join(dir, 'input.tsx'), transformed)
    execFileSync(
      process.execPath,
      // Same reasoning as the Web renderer: transpile only, automatic JSX
      // runtime, because the generated code carries identifiers from the
      // original module and has no `React` in scope.
      [tscBinary(), path.join(dir, 'input.tsx'), '--jsx', 'react-jsx', '--module', 'commonjs',
       '--target', 'es2022', '--outDir', dir, '--types', '', '--noCheck'],
      { encoding: 'utf8', stdio: 'pipe' },
    )

    const compiled = readFileSync(path.join(dir, 'input.js'), 'utf8')
    const exports: Record<string, unknown> = {}
    new Function('require', 'exports', compiled)(nativeRequire, exports)

    interface TestInstance {
      props: Record<string, unknown>
    }
    interface TestRoot {
      toJSON: () => Tree
      root: { findAll: (predicate: (node: TestInstance) => boolean) => TestInstance[] }
    }
    const renderer = require('react-test-renderer') as {
      create: (element: unknown) => TestRoot
      act: (callback: () => void) => void
    }
    const { createElement } = require('react') as { createElement: (c: unknown) => unknown }

    const globals = globalThis as Record<string, unknown>
    const restore = Object.keys(scope).map((key) => [key, globals[key]] as const)
    Object.assign(globals, scope)
    try {
      // React 19 doesn't flush outside `act`, so `create` alone yields an
      // empty tree -- indistinguishable from a component that rendered
      // nothing, which is how this first appeared. `toJSON` has to be read
      // *after* the callback, not inside it: within it the commit hasn't
      // happened yet and the answer is still null.
      let root: TestRoot | null = null
      renderer.act(() => {
        root = renderer.create(createElement(exports[componentName]))
      })
      if (root === null) return null
      const mounted = root as TestRoot
      for (const pass of layoutPasses) {
        const targets = mounted.root.findAll((node) => typeof node.props.onLayout === 'function')
        if (targets.length !== pass.length) {
          throw new Error(`layout fixture supplied ${pass.length} boxes for ${targets.length} targets`)
        }
        renderer.act(() => {
          targets.forEach((target, index) => {
            const onLayout = target.props.onLayout as (event: unknown) => void
            onLayout({ nativeEvent: { layout: pass[index] } })
          })
        })
      }
      return mounted.toJSON()
    } finally {
      for (const [key, value] of restore) globals[key] = value
    }
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

/// The generated module's own requires. Nothing is intercepted here: the
/// resolve hook above already redirects `react-native` and the two Dowel
/// packages, and it has to, since those packages import each other through
/// Node's loader rather than through anything handed to this module.
function nativeRequire(id: string): unknown {
  return require(id)
}
