import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'
import { gzipSync } from 'node:zlib'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const assets = path.join(root, 'dist', 'assets')
const javascript = readdirSync(assets).filter((name) => name.endsWith('.js'))
if (javascript.length === 0) throw new Error('Vite production build emitted no JavaScript')

const bytes = javascript.map((name) => readFileSync(path.join(assets, name)))
const raw = bytes.reduce((total, value) => total + value.length, 0)
const gzip = bytes.reduce((total, value) => total + gzipSync(value).length, 0)
if (raw > 220_000 || gzip > 70_000) {
  throw new Error(`Web production bundle grew unexpectedly: raw=${raw}, gzip=${gzip}`)
}
console.log(`Web production bundle check passed (raw=${raw}, gzip=${gzip})`)
