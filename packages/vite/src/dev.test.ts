// The dev-server half of the plugin, which nothing had ever run.
//
// Every example verifies a *build*: one pass, one theme load, one project
// scan, exit. Dev is a different program -- it stays up, the project
// changes underneath it, and the parts that handle that (`watchChange`,
// the per-transform rescan, the candidate-stylesheet invalidation) had no
// coverage at all. Code that only ever type-checks is the failure mode
// this repository keeps finding.
//
// A real `createServer`, not a mock: what is under test is the plugin's
// agreement with Vite, and a mock would be a copy of my belief about it.

import assert from 'node:assert/strict'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'
import { createServer, type ViteDevServer } from 'vite'

import { hozo } from './index.ts'

const roots: string[] = []
const servers: ViteDevServer[] = []

after(async () => {
  for (const server of servers) await server.close()
  for (const root of roots) rmSync(root, { recursive: true, force: true })
})

function project(files: Record<string, string>): string {
  const root = mkdtempSync(path.join(tmpdir(), 'hozo-dev-'))
  roots.push(root)
  for (const [name, content] of Object.entries(files)) {
    const file = path.join(root, name)
    mkdirSync(path.dirname(file), { recursive: true })
    writeFileSync(file, content)
  }
  return root
}

async function serve(root: string) {
  const server = await createServer({
    root,
    configFile: false,
    logLevel: 'silent',
    server: { middlewareMode: true, watch: null },
    // The temporary project has no node_modules of its own, so React's
    // JSX runtime is resolved from this repository's. Vite's own JSX
    // transform then runs exactly as it would in a real project -- which
    // matters, because it is what reads the output Hozo produced.
    resolve: {
      alias: {
        'react/jsx-dev-runtime': createRequire(import.meta.url).resolve('react/jsx-dev-runtime'),
        'react/jsx-runtime': createRequire(import.meta.url).resolve('react/jsx-runtime'),
        react: createRequire(import.meta.url).resolve('react'),
      },
    },
    plugins: [hozo()],
  })
  servers.push(server)
  return server
}

const candidates = (root: string) =>
  readFileSync(path.join(root, 'node_modules', '.hozo', 'candidates.css'), 'utf8')

const APP = `import { View } from '@hozo/core'
import { accent } from './accent.ts'
export const App = () => <View className={accent()}><View className="p-4" /></View>
`

test('the dev server lowers a module on request', async () => {
  const root = project({ 'App.tsx': APP, 'accent.ts': "export const accent = () => 'bg-emerald-500'\n" })
  const server = await serve(root)

  const result = await server.transformRequest('/App.tsx')
  assert.ok(result, 'the module was not transformed')
  assert.match(result.code, /hozo-view/, 'the View did not lower')
  assert.match(result.code, /App\.tsx\.hozo\.css/, 'the companion stylesheet was not imported')

  const companion = readFileSync(path.join(root, 'App.tsx.hozo.css'), 'utf8')
  assert.match(companion, /padding-top: 16px/)
})

test('a class only a helper produces reaches the candidate stylesheet', async () => {
  const root = project({ 'App.tsx': APP, 'accent.ts': "export const accent = () => 'bg-emerald-500'\n" })
  await serve(root)
  assert.match(candidates(root), /\.bg-emerald-500/)
})

test('editing a helper updates the candidate stylesheet', async () => {
  // The rescan inside `transform`, which is what makes a class written
  // during `next dev`/`vite dev` appear without a restart. Driven through
  // the server, so the file has to be on disk with a newer mtime -- the
  // cache skips a file it believes it has already seen.
  const root = project({ 'App.tsx': APP, 'accent.ts': "export const accent = () => 'bg-emerald-500'\n" })
  const server = await serve(root)
  assert.match(candidates(root), /\.bg-emerald-500/)

  const helper = path.join(root, 'accent.ts')
  writeFileSync(helper, "export const accent = () => 'bg-rose-500'\n")
  await server.transformRequest('/accent.ts')

  const css = candidates(root)
  assert.match(css, /\.bg-rose-500/, 'the new class never reached the stylesheet')
})

test('deleting a file takes its classes with it', async () => {
  // Nothing else revisits an entry that stopped being scanned, so without
  // `watchChange` a deleted file's classes would stay in the stylesheet
  // for as long as the cache file survives.
  const root = project({ 'App.tsx': APP, 'accent.ts': "export const accent = () => 'bg-emerald-500'\n" })
  const server = await serve(root)
  assert.match(candidates(root), /\.bg-emerald-500/)

  const helper = path.join(root, 'accent.ts')
  rmSync(helper)
  const plugin = server.config.plugins.find((p) => p.name === 'hozo')
  assert.ok(plugin?.watchChange, 'the plugin no longer handles watchChange')
  await (plugin.watchChange as (id: string, change: { event: string }) => unknown).call(
    plugin,
    helper,
    { event: 'delete' },
  )

  assert.doesNotMatch(candidates(root), /\.bg-emerald-500/, 'the deleted file kept its classes')
})

test('a file created while the server is up is scanned', async () => {
  const root = project({ 'App.tsx': APP, 'accent.ts': "export const accent = () => 'bg-emerald-500'\n" })
  const server = await serve(root)

  const added = path.join(root, 'later.ts')
  writeFileSync(added, "export const other = () => 'bg-amber-500'\n")
  const plugin = server.config.plugins.find((p) => p.name === 'hozo')
  await (plugin!.watchChange as (id: string, change: { event: string }) => unknown).call(
    plugin,
    added,
    { event: 'create' },
  )
  await server.transformRequest('/later.ts')

  assert.match(candidates(root), /\.bg-amber-500/, 'a file created after startup was never scanned')
})

test('editing a component rewrites its stylesheet, and only when it changed', async () => {
  // The companion `.hozo.css` is written *next to the source*, inside the
  // project the watcher is watching. So a transform writes a file that
  // can trigger another transform, and the thing standing between that and
  // a loop is `writeFileIfChanged` refusing to rewrite identical bytes.
  const root = project({
    'App.tsx': "import { View } from '@hozo/core'\nexport const App = () => <View className=\"p-4\" />\n",
  })
  const server = await serve(root)
  const companion = path.join(root, 'App.tsx.hozo.css')

  await server.transformRequest('/App.tsx')
  assert.match(readFileSync(companion, 'utf8'), /padding-top: 16px/)

  // The same module transformed again with unchanged source, which is
  // ordinary in dev -- an unrelated HMR round invalidates it and it comes
  // back through. The stylesheet must not be rewritten, or that transform
  // would invalidate the stylesheet it just wrote and start the next
  // round.
  //
  // `invalidateAll` is what makes this a test: without it Vite serves the
  // cached result, the plugin never runs, and the assertion holds for a
  // reason that has nothing to do with Hozo.
  const before = statSync(companion).mtimeMs
  server.moduleGraph.invalidateAll()
  await server.transformRequest('/App.tsx')
  assert.equal(statSync(companion).mtimeMs, before, 'an unchanged module rewrote its stylesheet')

  // A real edit does reach it.
  writeFileSync(
    path.join(root, 'App.tsx'),
    "import { View } from '@hozo/core'\nexport const App = () => <View className=\"p-8\" />\n",
  )
  server.moduleGraph.invalidateAll()
  await server.transformRequest('/App.tsx')
  assert.match(readFileSync(companion, 'utf8'), /padding-top: 32px/, 'the edit never reached the CSS')
})
