import assert from 'node:assert/strict'
import { test } from 'node:test'

import { nativePackageName } from './native-loader.ts'
import { NATIVE_TARGETS, cdylibFileName, hostTarget, publishManifest } from './native-targets.ts'

test('the packer and the loader name the same packages', () => {
  // The whole reason the table exists. These are two independent
  // derivations of one fact -- the packer goes from a Rust triple, the
  // loader from `process.platform`/`process.arch` at runtime -- and a
  // disagreement is invisible until someone on the odd platform installs a
  // published package and is told no addon could be loaded for it.
  for (const target of NATIVE_TARGETS) {
    assert.equal(
      nativePackageName(target.platform, target.arch, target.libc ?? 'gnu'),
      target.packageName,
      `${target.triple} is packed as ${target.packageName} and looked up as something else`,
    )
  }
})

test('every platform the loader can name is one we build', () => {
  // The other direction, which the loop above cannot see: a platform the
  // loader knows how to ask for and nothing publishes is a promise the
  // install cannot keep.
  const packed = new Set(NATIVE_TARGETS.map((target) => target.packageName))
  for (const platform of ['win32', 'darwin', 'linux'] as NodeJS.Platform[]) {
    for (const arch of ['x64', 'arm64']) {
      for (const libc of ['gnu', 'musl'] as const) {
        const name = nativePackageName(platform, arch, libc)
        if (name === undefined) continue
        assert.ok(packed.has(name), `the loader asks for ${name} and no target builds it`)
      }
    }
  }
})

test('names the file Cargo actually writes', () => {
  // Unix prefixes a cdylib with `lib` and Windows does not.
  assert.equal(cdylibFileName('win32', 'hozo_napi'), 'hozo_napi.dll')
  assert.equal(cdylibFileName('darwin', 'hozo_napi'), 'libhozo_napi.dylib')
  assert.equal(cdylibFileName('linux', 'hozo_napi'), 'libhozo_napi.so')
  assert.throws(() => cdylibFileName('freebsd', 'hozo_napi'))
})

test('finds the host target, including the libc split', () => {
  assert.equal(hostTarget('win32', 'x64')?.triple, 'x86_64-pc-windows-msvc')
  assert.equal(hostTarget('linux', 'x64', 'musl')?.triple, 'x86_64-unknown-linux-musl')
  assert.equal(hostTarget('linux', 'x64', 'gnu')?.triple, 'x86_64-unknown-linux-gnu')
  // A platform with no libc question ignores the argument rather than
  // failing to match on it.
  assert.equal(hostTarget('darwin', 'arm64', 'musl')?.triple, 'aarch64-apple-darwin')
  assert.equal(hostTarget('freebsd', 'x64'), undefined)
})

test('the published manifest lists every platform, pinned exactly', () => {
  // Generated at release time rather than committed: a manifest listing
  // eight packages that do not exist on the registry yet makes
  // `pnpm install` fail for anyone working in this repository. This is the
  // only place the generated shape is checked.
  const published = publishManifest({ name: '@hozo/compiler', version: '1.2.3', private: true })

  assert.equal(published.private, undefined, 'the published manifest must not be private')
  for (const target of NATIVE_TARGETS) {
    assert.equal(
      published.optionalDependencies[target.packageName],
      '1.2.3',
      `${target.packageName} is not pinned to the release version`,
    )
  }
  assert.equal(Object.keys(published.optionalDependencies).length, NATIVE_TARGETS.length)
})
