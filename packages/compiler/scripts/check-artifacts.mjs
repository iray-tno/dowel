// Refuses to publish an incomplete set.
//
// The failure this prevents is quiet on the machine doing the publishing
// and loud on someone else's: `@hozo/compiler` lists all eight platform
// packages as *optional* dependencies, so npm skipping one it cannot find
// is indistinguishable from npm skipping the seven it cannot use. The
// install succeeds either way, and the first thing that happens on the
// platform whose build failed is "no native addon could be loaded".
//
// Run in the release workflow between downloading the build artifacts and
// publishing them.

import { existsSync, readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { NATIVE_TARGETS } from '../src/native-targets.ts'

const here = path.dirname(fileURLToPath(import.meta.url))
const packageDir = path.resolve(here, '..')
const repoRoot = path.resolve(packageDir, '..', '..')

function argument(name) {
  const index = process.argv.indexOf(name)
  return index === -1 ? undefined : process.argv[index + 1]
}

const artifacts = path.resolve(argument('--artifacts') ?? path.join(repoRoot, 'artifacts'))
const { version } = JSON.parse(readFileSync(path.join(packageDir, 'package.json'), 'utf8'))

if (!existsSync(artifacts)) {
  throw new Error(`no artifacts directory at ${artifacts}`)
}

const problems = []
for (const target of NATIVE_TARGETS) {
  const dir = path.join(artifacts, target.packageName.replace('@hozo/', ''))
  if (!existsSync(dir)) {
    problems.push(`${target.packageName}: nothing was built for ${target.triple}`)
    continue
  }
  const manifestPath = path.join(dir, 'package.json')
  if (!existsSync(manifestPath)) {
    problems.push(`${target.packageName}: no package.json in ${dir}`)
    continue
  }
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))
  if (manifest.name !== target.packageName) {
    problems.push(`${dir}: names itself ${manifest.name}, expected ${target.packageName}`)
  }
  // Every package in one release has to carry the same version, or a
  // machine installs a binding built from different source than the
  // JavaScript that calls it.
  if (manifest.version !== version) {
    problems.push(`${target.packageName}: version ${manifest.version}, expected ${version}`)
  }
  if (!existsSync(path.join(dir, manifest.main))) {
    problems.push(`${target.packageName}: ${manifest.main} is missing`)
  }
}

const extra = existsSync(artifacts)
  ? readdirSync(artifacts, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => `@hozo/${entry.name}`)
      .filter((name) => !NATIVE_TARGETS.some((target) => target.packageName === name))
  : []
for (const name of extra) {
  // Not fatal on its own, but it means the target table and whatever
  // produced this disagree, and the next thing to check is which.
  problems.push(`${name}: built, but not in the target table`)
}

if (problems.length > 0) {
  throw new Error(`release artifacts are incomplete:\n${problems.map((p) => `  ${p}`).join('\n')}`)
}

console.log(`all ${NATIVE_TARGETS.length} platform packages present at ${version}`)
