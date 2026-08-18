// Rewrites `@hozo/compiler`'s manifest into the one that gets published.
//
// The optional dependencies are generated rather than committed, and that
// is not tidiness: a manifest listing eight packages that do not exist on
// the registry yet makes `pnpm install` fail for anyone working in this
// repository. They come into existence at the moment of the release, so
// that is when the list is written.
//
// `publishManifest` itself lives beside the target table it is derived
// from. This file is only the command -- it was both for about ten
// minutes, and importing it from a test rewrote the real manifest.
//
//   node scripts/prepare-publish.mjs

import { readFileSync, writeFileSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

import { NATIVE_TARGETS, publishManifest } from '../src/native-targets.ts'

const here = path.dirname(fileURLToPath(import.meta.url))
const manifestPath = path.join(here, '..', 'package.json')
const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'))

writeFileSync(manifestPath, `${JSON.stringify(publishManifest(manifest), null, 2)}\n`)
console.log(
  `prepared @hozo/compiler@${manifest.version} for publishing with ` +
    `${NATIVE_TARGETS.length} optional platform dependencies`,
)
