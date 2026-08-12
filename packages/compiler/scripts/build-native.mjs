// Dev-only build step: compiles the dowel_napi crate and copies the
// resulting native addon here as dowel_napi.node, so `src/index.ts` can
// require() it directly.
//
// This is a placeholder for proper @napi-rs/cli packaging (cross-platform
// prebuilds, per-platform npm packages) -- deliberately deferred until a
// published package actually needs it (see the dowel_napi commit that
// introduced this binding). Windows-only right now: extend the
// extension-per-platform map below when that becomes necessary.

import { execFileSync } from 'node:child_process'
import { copyFileSync, existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import path from 'node:path'

const here = path.dirname(fileURLToPath(import.meta.url))
const repoRoot = path.resolve(here, '..', '..', '..')

const platformExtensions = {
  win32: 'dll',
  darwin: 'dylib',
  linux: 'so',
}

const ext = platformExtensions[process.platform]
if (!ext) {
  throw new Error(`build-native.mjs doesn't know the cdylib extension for platform "${process.platform}" yet`)
}

execFileSync('cargo', ['build', '-p', 'dowel_napi'], { cwd: repoRoot, stdio: 'inherit' })

const built = path.join(repoRoot, 'target', 'debug', `dowel_napi.${ext}`)
if (!existsSync(built)) {
  throw new Error(`expected build output at ${built}, but it doesn't exist`)
}

const dest = path.join(here, '..', 'dowel_napi.node')
copyFileSync(built, dest)
console.log(`copied ${built} -> ${dest}`)
