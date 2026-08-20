// The publishing metadata for every package, in one place.
//
// Nine packages need the same eight fields to agree, and a registry is
// unforgiving about disagreement: a wrong `exports` path produces a
// package that installs cleanly and imports nothing, and npm has no undo
// past 72 hours. Keeping the shape here rather than in nine files means
// the answer to "what does a Hozo package look like" has one place to be
// wrong in; `check-packages.mjs` re-derives it and fails on any hand edit
// that drifted, then looks inside the tarballs to see whether the answer
// was true.
//
//   node scripts/package-metadata.mjs        # write
//   node scripts/package-metadata.mjs --check # verify, exit 1 on drift
//
// Deliberately *not* a generator that owns the whole file: each package's
// dependencies, peer ranges and scripts are its own business. This writes
// only the fields listed in `SHARED` and `PACKAGES`.

import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

/** The version every package publishes at. They release in lockstep. */
export const VERSION = '0.1.0'

const REPOSITORY = 'https://github.com/iray-tno/dowel'

/**
 * Per-package entry points and keywords.
 *
 * `main`/`exports` name `./dist` rather than `./src`, in the workspace as
 * well as in the tarball. The alternative -- source in the workspace,
 * `dist` swapped in by `publishConfig` at publish time -- keeps the dev
 * loop a step shorter, and was rejected twice over. `@hozo/core` and
 * `@hozo/a11y` are `.tsx`, and Node's type stripping does not transform
 * JSX, so those two cannot be loaded from source at all. And the two bugs
 * this build's first type-check turned up had both survived by the
 * published shape never being the shape under test.
 */
const PACKAGES = {
  compiler: {
    exports: {
      '.': './dist/index.js',
      './project': './dist/project.js',
      './lower': './dist/lower.js',
      './diagnostics': './dist/diagnostics.js',
      './sources': './dist/sources.js',
    },
    keywords: ['react-native', 'compiler', 'rust', 'napi', 'tailwind'],
  },
  core: {
    exports: { '.': './dist/index.js' },
    keywords: ['react-native', 'react', 'components', 'accessibility', 'universal'],
  },
  runtime: {
    exports: { '.': './dist/index.js' },
    // Metro resolves `.native.js` by filename suffix only while a package
    // has no `exports`; once it does, conditions take over and the suffix
    // is ignored. Naming the condition is what keeps the native build
    // reaching `index.native.js` instead of the DOM one.
    native: true,
    keywords: ['react-native', 'runtime', 'styles', 'animation'],
  },
  a11y: {
    exports: { '.': './dist/index.js' },
    native: true,
    keywords: ['react-native', 'accessibility', 'a11y', 'aria', 'dialog'],
  },
  tailwind: {
    exports: { '.': './dist/index.js' },
    keywords: ['tailwindcss', 'theme', 'design-tokens', 'react-native'],
  },
  vite: {
    exports: { '.': './dist/index.js' },
    keywords: ['vite', 'vite-plugin', 'react-native', 'react-native-web'],
  },
  next: {
    // `loader.js` stays plain JavaScript at the package root: a webpack or
    // Turbopack loader is resolved and executed by the bundler, which is
    // not going to compile it first.
    exports: { '.': './dist/index.js', './loader': './loader.js' },
    files: ['dist', 'loader.js'],
    keywords: ['nextjs', 'next', 'webpack', 'turbopack', 'react-native-web'],
  },
  metro: {
    exports: {
      '.': './dist/index.js',
      './config': './dist/config.js',
      './project': './dist/project.js',
    },
    keywords: ['metro', 'react-native', 'expo', 'transformer'],
  },
  storybook: {
    // Same reason as the Next loader: Storybook reads `preset.js` itself.
    exports: { '.': './preset.js', './preset': './preset.js' },
    files: ['dist', 'preset.js'],
    keywords: ['storybook', 'storybook-addon', 'react-native', 'vite'],
  },
}

/** Fields every package gets, identically. */
function shared(name) {
  return {
    version: VERSION,
    license: 'MIT',
    // `workspace:^` rather than `workspace:*` throughout. pnpm turns the
    // first into `^0.1.0` and the second into an exact `0.1.0`; an exact
    // pin means a project holding two Hozo packages one patch apart gets
    // two copies of the compiler, and so two native addons and two caches.
    repository: { type: 'git', url: `git+${REPOSITORY}.git`, directory: `packages/${name}` },
    homepage: `${REPOSITORY}/tree/main/packages/${name}#readme`,
    bugs: { url: `${REPOSITORY}/issues` },
    publishConfig: { access: 'public', provenance: true },
  }
}

/** The full metadata a package's `package.json` must carry. */
export function metadataFor(name) {
  const spec = PACKAGES[name]
  if (!spec) throw new Error(`no metadata defined for packages/${name}`)
  const exportsField = {}
  for (const [subpath, target] of Object.entries(spec.exports)) {
    const types = target.replace(/\.js$/, '.d.ts')
    // `types` first: the resolver takes the first matching condition, and
    // a `default` ahead of it wins for a TypeScript consumer too.
    exportsField[subpath] = target.startsWith('./dist')
      ? spec.native && subpath === '.'
        ? {
            types,
            'react-native': target.replace(/\.js$/, '.native.js'),
            default: target,
          }
        : { types, default: target }
      : target
  }
  // An `exports` map is a closed door: every subpath not named in it stops
  // resolving, `./package.json` included. Metro reads it, and so does the
  // native-render harness here -- both broke the moment these packages
  // gained an `exports` field at all. Re-exporting it is the standard
  // answer and costs nothing.
  exportsField['./package.json'] = './package.json'
  const main = spec.exports['.']
  return {
    ...shared(name),
    main,
    types: main.startsWith('./dist') ? main.replace(/\.js$/, '.d.ts') : undefined,
    exports: exportsField,
    // `src` ships alongside `dist` because the build emits
    // `declarationMap` and `sourceMap`, and a map whose source is absent
    // points nowhere. With it, a stack trace through Hozo lands on the
    // real line and "go to definition" reaches the commented original,
    // which for a compiler is worth the few kilobytes. Tests are excluded
    // by name -- npm honours negations here, and they are the only thing
    // under `src` that nobody installing this wants.
    files: [...(spec.files ?? ['dist']), 'src', '!src/**/*.test.ts', '!src/**/*.test.tsx'],
    keywords: spec.keywords,
  }
}

export const PACKAGE_NAMES = Object.keys(PACKAGES)

/** The order `package.json` keys are written in, for a readable diff. */
const KEY_ORDER = [
  'name', 'version', 'description', 'keywords', 'license', 'repository',
  'homepage', 'bugs', 'type', 'main', 'types', 'exports', 'files',
  'publishConfig', 'scripts', 'dependencies', 'peerDependencies',
  'peerDependenciesMeta', 'devDependencies',
]

function ordered(json) {
  const out = {}
  for (const key of KEY_ORDER) if (key in json) out[key] = json[key]
  for (const key of Object.keys(json)) if (!(key in out)) out[key] = json[key]
  return out
}

/** `package.json` for `name`, with the shared metadata applied. */
export function applyMetadata(name) {
  const file = path.join(root, 'packages', name, 'package.json')
  const json = JSON.parse(readFileSync(file, 'utf8'))
  delete json.private
  const merged = { ...json, ...metadataFor(name) }
  for (const key of Object.keys(merged)) if (merged[key] === undefined) delete merged[key]
  // Workspace dependencies all use the caret protocol; see `shared`.
  for (const field of ['dependencies', 'devDependencies']) {
    for (const [dep, range] of Object.entries(merged[field] ?? {})) {
      if (range.startsWith('workspace:')) merged[field][dep] = 'workspace:^'
    }
  }
  return { file, text: JSON.stringify(ordered(merged), null, 2) + '\n' }
}

if (import.meta.filename === process.argv[1]) {
  const check = process.argv.includes('--check')
  let drifted = 0
  for (const name of PACKAGE_NAMES) {
    const { file, text } = applyMetadata(name)
    if (check) {
      if (readFileSync(file, 'utf8') !== text) {
        console.error(`drift: packages/${name}/package.json`)
        drifted += 1
      }
    } else {
      writeFileSync(file, text)
    }
  }
  if (check && drifted > 0) {
    console.error(`\n${drifted} package.json out of date. Run: node scripts/package-metadata.mjs`)
    process.exit(1)
  }
  console.log(check ? 'package metadata up to date' : `wrote ${PACKAGE_NAMES.length} package.json`)
}
