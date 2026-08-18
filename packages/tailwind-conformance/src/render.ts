// Renders the Web backend's output and returns the DOM it produces.
//
// Everything else in this package compares text: Hozo's CSS against
// Tailwind's CSS, Hozo's styles against React Native's types. None of it
// establishes that the generated component *runs* -- that the JSX parses,
// that it mounts, that the class the CSS defines is the class that reaches
// the element. A compiler whose output has never been executed has a gap
// there that no amount of string comparison closes.
//
// Static markup rather than a real DOM: the question is what the component
// renders, not how it behaves, and `renderToStaticMarkup` answers it
// without a jsdom in the tree.

import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)

function packageRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
}

export interface Component {
  name: string
  /** The JSX expression `compile()` returned. */
  jsx: string
}

export interface Rendered {
  name: string
  html: string
  /** Every `class` the rendered markup actually carries. */
  classes: Set<string>
}

/**
 * Compiles and renders each component, in one pass.
 *
 * Transpiled by the `tsc` binary rather than a JavaScript API: TypeScript 7
 * doesn't expose `transpileModule` from the package entry any more, and the
 * binary is already how the type-check runs. One process for the whole
 * batch, since starting it is the expensive part.
 *
 * The automatic JSX runtime matters here: Hozo's generated code has no
 * `React` in scope, and requiring one would mean testing something other
 * than what the compiler emits.
 */
export function renderWeb(
  components: Component[],
  // Values for the identifiers the generated JSX carries over from the
  // original module -- an `onPress={save}` refers to a `save` that lives
  // there, not here. Something has to stand in for that module, and a
  // global is the least machinery that can.
  scope: Record<string, unknown> = {},
): Rendered[] {
  const dir = mkdtempSync(path.join(packageRoot(), '.render-'))
  try {
    const source = components
      .map(({ name, jsx }) => `export function ${name}() { return ${jsx}; }`)
      .join('\n')
    writeFileSync(path.join(dir, 'input.tsx'), source)

    const tsc = path.join(path.dirname(require.resolve('typescript/package.json')), 'bin', 'tsc')
    execFileSync(
      process.execPath,
      [
        tsc,
        path.join(dir, 'input.tsx'),
        '--jsx',
        'react-jsx',
        '--module',
        'commonjs',
        '--target',
        'es2022',
        '--outDir',
        dir,
        '--types',
        '',
        // Transpile only. The generated JSX carries expressions from the
        // original source verbatim -- an `onPress={save}` refers to a
        // `save` that lives in the user's module, not here -- so type
        // checking this in isolation asks a question with no answer. What
        // is being tested is that it parses and renders.
        '--noCheck',
      ],
      { encoding: 'utf8', stdio: 'pipe' },
    )

    const compiled = readFileSync(path.join(dir, 'input.js'), 'utf8')
    const exports: Record<string, unknown> = {}
    // A plain function rather than a module loader: the transpiled text is
    // CommonJS whose only dependency is the JSX runtime, so a `require` and
    // an `exports` are the whole environment it needs.
    new Function('require', 'exports', compiled)(require, exports)

    const { renderToStaticMarkup } = require('react-dom/server') as {
      renderToStaticMarkup: (element: unknown) => string
    }
    const { createElement } = require('react') as { createElement: (c: unknown) => unknown }

    const globals = globalThis as Record<string, unknown>
    const restore = Object.keys(scope).map((key) => [key, globals[key]] as const)
    Object.assign(globals, scope)
    try {
      return components.map(({ name }) => {
        const html = renderToStaticMarkup(createElement(exports[name]))
        return { name, html, classes: classesIn(html) }
      })
    } finally {
      for (const [key, value] of restore) globals[key] = value
    }
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

function classesIn(html: string): Set<string> {
  const classes = new Set<string>()
  for (const match of html.matchAll(/class="([^"]*)"/g)) {
    for (const name of match[1].split(/\s+/)) {
      if (name !== '') classes.add(name)
    }
  }
  return classes
}

/**
 * The class names a stylesheet writes rules for.
 *
 * Escapes are unwrapped, because Tailwind-shaped names reach CSS escaped
 * (`.hover\:bg-blue-500`) and reach the DOM as they were authored -- so
 * comparing the two without this would report every one of them as missing.
 */
export function classesDefinedIn(css: string): Set<string> {
  const classes = new Set<string>()
  // Selectors only -- each run of text that ends at an opening brace.
  // Scanning the whole sheet instead also matched the decimal point in
  // `oklch(0.623 ...)` and reported `623` as a class; anchoring the run to
  // the *end* of the previous rule instead missed every selector nested in
  // an `@media`, which is every responsive and `dark:` variant.
  for (const rule of css.matchAll(/([^{}]*)\{/g)) {
    for (const match of rule[1].matchAll(/\.((?:[\w-]|\\.)+)/g)) {
      classes.add(match[1].replace(/\\(.)/g, '$1'))
    }
  }
  return classes
}
