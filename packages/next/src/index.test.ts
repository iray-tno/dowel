import assert from 'node:assert/strict'
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import path from 'node:path'
import { after, test } from 'node:test'

import { withHozo } from './index.ts'

const roots: string[] = []

function project(source: string): string {
  const root = mkdtempSync(path.join(tmpdir(), 'hozo-next-'))
  roots.push(root)
  mkdirSync(path.join(root, 'src'), { recursive: true })
  writeFileSync(path.join(root, 'src', 'page.tsx'), source)
  return root
}

after(() => {
  for (const root of roots) rmSync(root, { recursive: true, force: true })
})

interface WebpackRule {
  enforce?: string
  test?: RegExp
  use?: { loader: string; options: { root: string; candidateCssPath: string } }[]
}

test('registers the loader with both of Next\'s bundlers', () => {
  const config = withHozo({}, { root: project('export const x = 1\n') }) as {
    turbopack: { rules: Record<string, { loaders: { loader: string }[]; as: string }> }
    webpack: (config: { module?: { rules?: WebpackRule[] } }, context: unknown) => {
      module: { rules: WebpackRule[] }
    }
  }

  const rule = config.turbopack.rules['*.tsx']
  assert.equal(rule.as, '*.tsx')
  assert.match(rule.loaders[0].loader, /loader\.js$/)

  const webpack = config.webpack({}, {})
  assert.match(webpack.module.rules[0].use![0].loader, /loader\.js$/)
})

test("the webpack rule is `pre`, which is what makes it run before SWC", () => {
  // Regression: array position looks like it decides order and does not.
  // webpack runs a module's loaders right-to-left, so the rule this
  // prepends runs *last* without `enforce: 'pre'` -- and Hozo was handed
  // already-compiled JavaScript, found no JSX, and left the `@hozo/core`
  // import in a server component.
  const config = withHozo({}, { root: project('export const x = 1\n') }) as {
    webpack: (config: { module?: { rules?: WebpackRule[] } }, context: unknown) => {
      module: { rules: WebpackRule[] }
    }
  }
  const [rule] = config.webpack({}, {}).module.rules
  assert.equal(rule.enforce, 'pre')
  assert.ok(rule.test!.test('page.tsx'))
  assert.ok(!rule.test!.test('page.ts'))
})

test('keeps whatever the project already configured', () => {
  let called = false
  const config = withHozo(
    {
      turbopack: { root: '/somewhere', rules: { '*.svg': { loaders: ['svg-loader'] } } },
      webpack(c: { module?: { rules?: WebpackRule[] } }) {
        called = true
        return c as { module: { rules: WebpackRule[] } }
      },
    },
    { root: project('export const x = 1\n') },
  ) as {
    turbopack: { root: string; rules: Record<string, unknown> }
    webpack: (config: unknown, context: unknown) => unknown
  }

  assert.equal(config.turbopack.root, '/somewhere')
  assert.ok(config.turbopack.rules['*.svg'], 'an existing Turbopack rule was dropped')
  assert.ok(config.turbopack.rules['*.tsx'], 'the Hozo rule is missing')
  config.webpack({}, {})
  assert.ok(called, "the project's own webpack() was not called")
})

test('writes the candidate stylesheet before anything is compiled', () => {
  // Turbopack has no build-start hook, so this happens while the config is
  // being evaluated. A module compiled before the file existed would be a
  // module importing a stylesheet that isn't there.
  const root = project("export const accent = () => 'bg-emerald-500'\n")
  const config = withHozo({}, { root }) as {
    turbopack: { rules: Record<string, { loaders: { options: { candidateCssPath: string } }[] }> }
  }
  const css = config.turbopack.rules['*.tsx'].loaders[0].options.candidateCssPath
  assert.match(readFileSync(css, 'utf8'), /\.bg-emerald-500/)
})
