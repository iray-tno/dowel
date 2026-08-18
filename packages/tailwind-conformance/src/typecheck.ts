// Type-checks the Native backend's *output* against React Native's own
// types.
//
// The refusal audit checks the denominator: for everything Hozo declines
// to lower, it asks React Native's types whether that refusal is honest.
// This is the other half, and it had been missing the whole time -- for
// everything Hozo *does* lower, nothing ever checked that the style it
// emits is one React Native would accept. Every key and value was the
// compiler's own say-so, verified against a surface this repo extracts with
// a regex.
//
// Handing the generated `StyleSheet` to `tsc` replaces both halves of that
// with React Native's actual declarations: the key has to exist, the value
// has to be assignable, and the check is the same one an app would get.

import { execFileSync } from 'node:child_process'
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const require = createRequire(import.meta.url)

function packageRoot(): string {
  return path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
}

export interface TypeError {
  /** The candidate whose style produced it, if it could be attributed. */
  candidate?: string
  message: string
}

/**
 * A style is valid if React Native would accept it on *some* component, so
 * the three style types are a union rather than an intersection.
 *
 * An intersection would be the wrong question and would reject correct
 * output: `overflow` is narrower on `ImageStyle` than on `ViewStyle`, so
 * `overflow: 'scroll'` -- valid on a View -- fails an intersection. What is
 * being asked here is "could this style be used", which is exactly what a
 * union answers.
 */
const STYLE_TYPE = 'ViewStyle | TextStyle | ImageStyle'

/**
 * Runs every style entry through `tsc` and returns what React Native's
 * types reject.
 *
 * One file for all of them rather than one per candidate: `tsc` spends its
 * time loading React Native's declarations, so a file per candidate would
 * be thousands of times that for the same answer. Each entry is named after
 * its candidate so an error can be attributed back.
 */
export function typeCheckStyles(entries: { candidate: string; style: string }[]): TypeError[] {
  // Inside the package, not the OS temp directory: `react-native` has to
  // resolve from here, and from anywhere else it doesn't -- which makes
  // `ViewStyle` an `any` and every style pass. That is how the first run
  // of this checker reported 19,130 entries and zero errors while being
  // unable to reject anything at all.
  const dir = mkdtempSync(path.join(packageRoot(), '.typecheck-'))
  try {
    const declarations = entries
      .map(({ candidate, style }, index) => {
        // The candidate name goes in a comment, not in the identifier:
        // class names contain characters an identifier can't hold, and the
        // index is what the error line maps back to anyway.
        return `// ${candidate}\nexport const s${index}: ${STYLE_TYPE} = ${style};`
      })
      .join('\n')

    const source = `import type { ViewStyle, TextStyle, ImageStyle } from 'react-native';\n${declarations}\n`
    const file = path.join(dir, 'styles.ts')
    writeFileSync(file, source)
    writeFileSync(
      path.join(dir, 'tsconfig.json'),
      JSON.stringify({
        compilerOptions: {
          noEmit: true,
          strict: true,
          // React Native's own declarations don't pass `skipLibCheck: false`
          // cleanly, and their internal consistency isn't what's being
          // measured here -- Hozo's output against them is.
          skipLibCheck: true,
          moduleResolution: 'bundler',
          module: 'esnext',
          target: 'esnext',
          types: [],
        },
        files: ['styles.ts'],
      }),
    )

    return runTsc(dir, source)
  } finally {
    rmSync(dir, { recursive: true, force: true })
  }
}

function runTsc(dir: string, source: string): TypeError[] {
  // Located from the package rather than through a subpath import: recent
  // TypeScript releases don't export `./bin/tsc`, and the binary is the
  // entry point here rather than the API.
  const tsc = path.join(path.dirname(require.resolve('typescript/package.json')), 'bin', 'tsc')
  let output = ''
  try {
    execFileSync(process.execPath, [tsc, '--project', dir], { encoding: 'utf8' })
    return []
  } catch (error) {
    // `tsc` exits non-zero when it finds errors, which is the normal path
    // here rather than a failure to run.
    output = String((error as { stdout?: string }).stdout ?? '')
    if (output === '') throw error
  }

  const lines = source.split('\n')
  const errors: TypeError[] = []
  for (const line of output.split('\n')) {
    // `tsc` prints an absolute path, so the filename is matched where it
    // falls rather than anchored. Anchoring dropped every error, which --
    // together with the module not resolving -- is what made the first run
    // look clean.
    const match = /styles\.ts\((\d+),\d+\): error \w+: (.*)$/.exec(line.trim())
    if (!match) continue
    errors.push({
      candidate: attribute(lines, Number(match[1]) - 1),
      message: match[2],
    })
  }
  return errors
}

/** Walks back to the `// candidate` comment above the failing line. */
function attribute(lines: string[], index: number): string | undefined {
  for (let i = index; i >= 0; i--) {
    const comment = /^\/\/ (.+)$/.exec(lines[i] ?? '')
    if (comment) return comment[1]
  }
  return undefined
}
