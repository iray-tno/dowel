// Looks inside the tarballs npm would upload, rather than at the
// `package.json` that describes them.
//
// The failure this exists for: publishing with `dist/` unbuilt. Every
// field is correct, `npm publish` succeeds, the package installs cleanly,
// and the first `import` fails for everyone. npm has no undo past 72
// hours and none at all once something depends on it, so the check has to
// happen before the upload rather than after the bug report.
//
// `npm pack --dry-run --json` is the authority here: it applies `files`,
// the implicit includes (`package.json`, `README`, `LICENSE`) and the
// implicit excludes, and reports exactly what would go up.
//
//   node scripts/check-packages.mjs

import { execSync } from 'node:child_process'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { PACKAGE_NAMES, VERSION, applyMetadata } from './package-metadata.mjs'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const problems = []

function fail(pkg, message) {
  problems.push(`@hozo/${pkg}: ${message}`)
}

/** The paths `npm pack` would put in the tarball, relative and slash-separated. */
function packedFiles(dir) {
  // `execSync` with one command string rather than `execFileSync` with an
  // argument array. Node 25 refuses to spawn a `.cmd` without a shell, so
  // Windows needs one either way, and passing an array alongside `shell:
  // true` earns a deprecation warning -- the shell concatenates arguments
  // instead of escaping them. There is nothing to escape here.
  const output = execSync('npm pack --dry-run --json', {
    cwd: dir,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'ignore'],
  })
  return new Set(JSON.parse(output)[0].files.map((file) => file.path.replaceAll('\\', '/')))
}

/** Every file path an `exports` map points at, at any depth. */
function exportTargets(node, found = []) {
  if (typeof node === 'string') {
    if (node.startsWith('./')) found.push(node.slice(2))
  } else if (node && typeof node === 'object') {
    for (const value of Object.values(node)) exportTargets(value, found)
  }
  return found
}

for (const name of PACKAGE_NAMES) {
  const dir = path.join(root, 'packages', name)
  const json = JSON.parse(readFileSync(path.join(dir, 'package.json'), 'utf8'))
  const files = packedFiles(dir)

  // The metadata is generated; a hand edit that drifts from the generator
  // is a difference nobody chose.
  const { text } = applyMetadata(name)
  if (readFileSync(path.join(dir, 'package.json'), 'utf8') !== text) {
    fail(name, 'package.json differs from scripts/package-metadata.mjs -- rerun it')
  }

  // A scoped package defaults to a paid private publish. Without this,
  // `npm publish` stops with 402 Payment Required.
  if (json.publishConfig?.access !== 'public') fail(name, 'publishConfig.access is not "public"')
  if (json.private) fail(name, 'still marked private')
  if (json.version !== VERSION) fail(name, `version is ${json.version}, expected ${VERSION}`)

  // Every entry point has to be in the tarball, which is the whole point.
  for (const target of [json.main, json.types, ...exportTargets(json.exports)]) {
    if (!target) continue
    const relative = target.replace(/^\.\//, '')
    if (!files.has(relative)) fail(name, `${relative} is named as an entry point but is not packed`)
  }

  // A published package with no README is a blank page on npm.
  if (!files.has('README.md')) fail(name, 'no README.md in the tarball')
  if (!files.has('LICENSE')) fail(name, 'no LICENSE in the tarball')

  // Things that must never ship.
  for (const file of files) {
    if (/\.test\.tsx?$/.test(file)) fail(name, `${file} is a test and should not ship`)
    if (file.endsWith('.node')) {
      fail(name, `${file} is a platform-specific addon; it belongs in its own package`)
    }
    if (file.startsWith('tsconfig')) fail(name, `${file} should not ship`)
  }

  // `workspace:*` publishes as an exact pin, which gives a project holding
  // two Hozo packages one patch apart two copies of the compiler -- and so
  // two native addons and two candidate caches. `workspace:^` dedupes.
  for (const field of ['dependencies', 'peerDependencies']) {
    for (const [dep, range] of Object.entries(json[field] ?? {})) {
      if (range.startsWith('workspace:') && range !== 'workspace:^') {
        fail(name, `${field}.${dep} is "${range}"; use "workspace:^"`)
      }
    }
  }

  // Depending on something unpublishable produces a package that cannot
  // be installed from the registry at all.
  for (const dep of Object.keys(json.dependencies ?? {})) {
    if (!dep.startsWith('@hozo/')) continue
    const depName = dep.slice('@hozo/'.length)
    if (!PACKAGE_NAMES.includes(depName)) {
      fail(name, `depends on ${dep}, which is not published`)
    }
  }
}

if (problems.length > 0) {
  console.error(`${problems.length} problem(s) would reach the registry:\n`)
  for (const problem of problems) console.error(`  - ${problem}`)
  process.exit(1)
}
console.log(`${PACKAGE_NAMES.length} packages pack correctly`)
