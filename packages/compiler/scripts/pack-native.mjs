// Builds the native binding for one target and writes the npm package that
// carries it.
//
//   node scripts/pack-native.mjs                     # this machine's target
//   node scripts/pack-native.mjs --target <triple>   # a cross build
//   node scripts/pack-native.mjs --out <dir>         # where packages land
//
// One package per platform, each declaring `os`/`cpu`/`libc` so npm skips
// the seven a machine cannot use. `@hozo/compiler` lists all of them as
// optional dependencies, which is what makes "skips" mean "installs
// nothing and carries on" rather than "fails".
//
// Separate from `build-native.mjs`, which stays the dev step: that one
// builds a debug addon next to `src/` for the repository's own tests, and
// this one produces release artifacts for publishing. They share the
// target table and nothing else.

import { execFileSync } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { NATIVE_TARGETS, cdylibFileName, hostTarget } from '../src/native-targets.ts'

const here = path.dirname(fileURLToPath(import.meta.url))
const packageDir = path.resolve(here, '..')
const repoRoot = path.resolve(packageDir, '..', '..')

function argument(name) {
  const index = process.argv.indexOf(name)
  return index === -1 ? undefined : process.argv[index + 1]
}

const requested = argument('--target')
const target = requested
  ? NATIVE_TARGETS.find((entry) => entry.triple === requested)
  : hostTarget(process.platform, process.arch, detectLibc())
if (!target) {
  const known = NATIVE_TARGETS.map((entry) => `  ${entry.triple}`).join('\n')
  throw new Error(
    requested
      ? `Hozo does not build ${requested}. Known targets:\n${known}`
      : `Hozo does not build ${process.platform}/${process.arch} yet. Known targets:\n${known}`,
  )
}

/**
 * Which C library this machine has.
 *
 * The same question `native-loader.ts` asks at runtime, by the same means:
 * a glibc build records its runtime version in the process report and a
 * musl build has nothing to record.
 */
function detectLibc() {
  if (process.platform !== 'linux') return 'gnu'
  return process.report?.getReport()?.header?.glibcVersionRuntime ? 'gnu' : 'musl'
}

const { version } = JSON.parse(readFileSync(path.join(packageDir, 'package.json'), 'utf8'))
const outDir = path.resolve(argument('--out') ?? path.join(repoRoot, 'artifacts'))

console.log(`building ${target.triple} -> ${target.packageName}@${version}`)
execFileSync('cargo', ['build', '--release', '-p', 'hozo_napi', '--target', target.triple], {
  cwd: repoRoot,
  stdio: 'inherit',
})

// `--target` puts the output under `target/<triple>/release`, which is
// true even when the triple is the host's own.
const built = path.join(repoRoot, 'target', target.triple, 'release', cdylibFileName(target.platform, 'hozo_napi'))
if (!existsSync(built)) {
  throw new Error(`cargo reported success but ${built} does not exist`)
}

const dir = path.join(outDir, target.packageName.replace('@hozo/', ''))
mkdirSync(dir, { recursive: true })

const binary = 'hozo_napi.node'
copyFileSync(built, path.join(dir, binary))

// `libc` is only meaningful where the platform has more than one, and npm
// warns about the field on platforms that don't.
writeFileSync(
  path.join(dir, 'package.json'),
  `${JSON.stringify(
    {
      name: target.packageName,
      version,
      description: `Compiled Hozo compiler binding for ${target.platform}/${target.arch}${target.libc ? ` (${target.libc})` : ''}.`,
      license: 'MIT',
      main: binary,
      files: [binary],
      os: [target.platform],
      cpu: [target.arch],
      ...(target.libc ? { libc: [target.libc] } : {}),
    },
    null,
    2,
  )}\n`,
)

writeFileSync(
  path.join(dir, 'README.md'),
  `# ${target.packageName}\n\n` +
    `The Hozo compiler's native binding, built for ${target.platform}/${target.arch}` +
    `${target.libc ? ` against ${target.libc} libc` : ''}.\n\n` +
    `Not installed directly. \`@hozo/compiler\` lists every platform build as an\n` +
    `optional dependency and npm installs the one this machine can run.\n`,
)

console.log(`wrote ${dir}`)
