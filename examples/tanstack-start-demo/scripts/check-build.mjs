import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'

function filesUnder(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name)
    return entry.isDirectory() ? filesUnder(file) : [file]
  })
}

const artifacts = filesUnder('.output')
  .filter((file) => /\.(?:js|mjs|css)$/.test(file))
  .map((file) => ({ file, content: readFileSync(file, 'utf8') }))
const output = artifacts.map(({ content }) => content).join('\n')
const compiledRoute = artifacts.find(
  ({ file, content }) =>
    file.includes(`${path.sep}_ssr${path.sep}`) &&
    content.includes('Canonical primitives become semantic HTML'),
)?.content

if (output.includes('@hozo/core')) {
  throw new Error('TanStack Start output still imports @hozo/core')
}
if (!output.includes('Hozo + TanStack Start')) {
  throw new Error('TanStack Start output is missing the compiled route')
}
if (!output.includes('background-color')) {
  throw new Error('TanStack Start output is missing Hozo-generated CSS')
}
if (
  !compiledRoute?.includes('"h1"') ||
  !compiledRoute.includes('"button"') ||
  !compiledRoute.includes('hozo-r0-')
) {
  throw new Error('TanStack Start server output is missing semantic Hozo lowering')
}

console.log('TanStack Start SSR/client build check passed')
