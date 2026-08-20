// Type-checks the Web backend's output the way a consumer's build would.
//
// Hozo splices its JSX back into the user's own `.tsx`, so whatever it
// emits is checked by *their* `tsc`. Nothing here had ever asked whether
// it passes. It did not: `<div tabIndex="0">` was emitted for every
// interactive Pressable, and React types `tabIndex` as a `number`, so any
// project that type-checks its build -- which `next build` does by
// default -- got an error out of Hozo's own output.
//
// `render.ts` deliberately transpiles with `--noCheck`, and its reason is
// sound: the generated JSX carries expressions from the original module
// verbatim, and an `onPress={save}` refers to a `save` that does not exist
// here. The way past that is to declare those names rather than to give up
// on checking. They are declared `any`, which is exactly right for this
// question -- the free identifiers are the user's business, and what is
// under test is the shape of the JSX around them.

import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import type { Component } from './render.ts'

const require = createRequire(import.meta.url)

function packageRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
}

/** One `tsc` complaint, as `file(line,col): error TSxxxx: message`. */
export interface TypeError {
  line: string
}

/**
 * Runs `tsc --noEmit` over the compiled components.
 *
 * `freeNames` are the identifiers the generated JSX carries over from the
 * module it was compiled from; each is declared `any` so that referencing
 * one is not itself the error being reported.
 *
 * The temp directory lives inside this package so that `react` and
 * `@types/react` resolve by walking up to its `node_modules`.
 */
export function typeCheckWeb(
  components: Component[],
  freeNames: string[] = [],
  runtimeImports: string[] = [],
): TypeError[] {
  const dir = mkdtempSync(path.join(packageRoot(), '.typecheck-'))
  try {
    const declarations = freeNames.map((name) => `declare const ${name}: any`).join('\n')
    // A real import, not a declaration. The names Hozo reaches for are
    // Hozo's own, so a stand-in would check nothing -- this is what
    // establishes that `hozoActivateKeyDown` is genuinely assignable to
    // React's `onKeyDown`, which is the only reason to emit it.
    const imports =
      runtimeImports.length > 0
        ? `import { ${[...new Set(runtimeImports)].sort().join(', ')} } from '@hozo/runtime'\n`
        : ''
    const source = `${imports}${declarations}\n${components
      .map(({ name, jsx }) => `export function ${name}() { return ${jsx}; }`)
      .join('\n')}\n`
    writeFileSync(path.join(dir, 'input.tsx'), source)

    // The same options a React project gets from `create-next-app` or
    // `create-vite`, so a passing check here means the same thing there.
    writeFileSync(
      path.join(dir, 'tsconfig.json'),
      JSON.stringify(
        {
          compilerOptions: {
            target: 'es2023',
            lib: ['es2023', 'dom', 'dom.iterable'],
            module: 'nodenext',
            moduleResolution: 'nodenext',
            jsx: 'react-jsx',
            strict: true,
            noEmit: true,
            types: ['react'],
          },
          include: ['input.tsx'],
        },
        null,
        2,
      ),
    )

    const tsc = path.join(path.dirname(require.resolve('typescript/package.json')), 'bin', 'tsc')
    try {
      execFileSync(process.execPath, [tsc, '-p', dir], { encoding: 'utf8', stdio: 'pipe' })
      return []
    } catch (error) {
      // `tsc` exits non-zero with the diagnostics on stdout.
      const output = String((error as { stdout?: string }).stdout ?? '')
      return output
        .split('\n')
        .map((line) => line.trim())
        .filter((line) => line.includes('error TS'))
        .map((line) => ({ line }))
    }
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}
