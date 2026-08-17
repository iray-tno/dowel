import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { gzipSync } from 'node:zlib'
import { fileURLToPath } from 'node:url'

import Metro from 'metro'

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const outputDirectory = path.join(projectRoot, 'dist')
mkdirSync(outputDirectory, { recursive: true })

const config = await Metro.loadConfig({ cwd: projectRoot, resetCache: true })
const builds = [
  ['application', 'index.js'],
  ['dowel', 'bench-dowel.js'],
  ['native', 'bench-native.js'],
]

const sizes = {}
for (const [name, entry] of builds) {
  const output = path.join(outputDirectory, `${name}.production.bundle`)
  await Metro.runBuild(config, {
    entry,
    platform: 'android',
    minify: true,
    dev: false,
    out: output,
  })
  const bytes = readFileSync(`${output}.js`)
  sizes[name] = { raw: bytes.length, gzip: gzipSync(bytes).length }
}

const result = {
  mode: { platform: 'android', dev: false, minify: true },
  bytes: sizes,
  dowelIncrement: {
    raw: sizes.dowel.raw - sizes.native.raw,
    gzip: sizes.dowel.gzip - sizes.native.gzip,
  },
}
writeFileSync(path.join(outputDirectory, 'bundle-sizes.json'), `${JSON.stringify(result, null, 2)}\n`)

console.log(JSON.stringify(result, null, 2))

// This pair has the same React Native dependencies and UI structure. A
// large positive delta means build-time Dowel code or an accidental runtime
// dependency leaked into the application bundle.
if (result.dowelIncrement.raw > 5_000 || result.dowelIncrement.gzip > 1_500) {
  throw new Error(`Dowel production increment is unexpectedly large: ${JSON.stringify(result.dowelIncrement)}`)
}
