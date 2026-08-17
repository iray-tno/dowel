// Bundles the example with Metro.
//
// This is the end-to-end check the project didn't have: everything else
// verifies a piece in isolation -- the compiler's output as text, as types,
// as a rendered tree. This runs the real bundler over a real source file,
// so the transformer, the generated candidate module, the runtime imports
// and every module resolution have to actually work together.
//
// It still isn't a device. What it establishes is that the bundle builds.

import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import Metro from 'metro'

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const out = path.join(projectRoot, 'dist')
mkdirSync(out, { recursive: true })

const argument = (name, fallback) => {
  const index = process.argv.indexOf(name)
  return index === -1 ? fallback : process.argv[index + 1]
}
const production = process.argv.includes('--production')
const entry = argument('--entry', 'index.js')
const platform = argument('--platform', 'ios')
const outputName = argument('--out', production ? 'index.production.bundle' : 'index.bundle')
const outputPath = path.join(out, outputName)

// Never from cache. This bundle is a check, and a cached transform would
// answer for a version of the compiler that is no longer there -- which it
// did, reporting a fixed bug as still broken.
const config = await Metro.loadConfig({ cwd: projectRoot, resetCache: true })

await Metro.runBuild(config, {
  entry,
  platform,
  minify: production,
  dev: !production,
  out: outputPath,
})

console.log(`bundled ${production ? 'production' : 'development'} ${platform} ->`, outputPath)
