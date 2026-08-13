// Tailwind v4 emits utilities in terms of its own custom properties
// (`padding: calc(var(--spacing) * 4)`), so comparing against Dowel's
// resolved pixel values means resolving those first. This reads the real
// `theme.css` shipped in the installed tailwindcss package rather than
// hardcoding a copy, so the numbers can't drift from the version under test.

import { createRequire } from 'node:module'
import { readFileSync } from 'node:fs'
import path from 'node:path'

const require = createRequire(import.meta.url)

export function tailwindPackageDir(): string {
  // `tailwindcss` has no main entry that resolves cleanly here, so locate
  // it via a file that definitely exists in the package root.
  return path.dirname(require.resolve('tailwindcss/theme.css'))
}

export function loadThemeVars(): Map<string, string> {
  const css = readFileSync(path.join(tailwindPackageDir(), 'theme.css'), 'utf8')
  const vars = new Map<string, string>()
  const re = /^\s*(--[a-z0-9-]+):\s*([^;]+);/gim
  let match: RegExpExecArray | null
  while ((match = re.exec(css))) {
    vars.set(match[1], match[2].trim())
  }
  return vars
}

export function tailwindVersion(): string {
  const pkg = JSON.parse(readFileSync(path.join(tailwindPackageDir(), 'package.json'), 'utf8'))
  return pkg.version
}
