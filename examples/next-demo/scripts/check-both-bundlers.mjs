// Both of Next.js's bundlers, because they are not interchangeable here.
//
// Turbopack is the default in Next 16 and webpack is still reachable with
// `--webpack`. Hozo registers the same loader with both, and that made the
// difference invisible until this ran: webpack applies a module's loaders
// right-to-left, so the rule Hozo prepends runs *last* unless it also says
// `enforce: 'pre'`. Under Turbopack the ordering was already right, so a
// Turbopack-only check passed while every webpack build was handed
// SWC-compiled JavaScript with no JSX left to lower.
import { execFileSync } from 'node:child_process'
import { rmSync } from 'node:fs'
import { createRequire } from 'node:module'

// Resolved to the package's own entry rather than the .bin shim: on
// Windows the shim is a .CMD, which spawnSync refuses without a shell.
const next = createRequire(import.meta.url).resolve('next/dist/bin/next')

for (const bundler of ['turbopack', 'webpack']) {
  rmSync('.next', { recursive: true, force: true })
  const args = ['build', ...(bundler === 'webpack' ? ['--webpack'] : [])]
  console.log(`\n=== next build (${bundler}) ===`)
  execFileSync(process.execPath, [next, ...args], { stdio: 'inherit' })
  execFileSync(process.execPath, ['scripts/check-build.mjs'], { stdio: 'inherit' })
}
