import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'

function filesUnder(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name)
    return entry.isDirectory() ? filesUnder(file) : [file]
  })
}

const artifacts = filesUnder('storybook-static')
  .filter((file) => /\.(?:js|css)$/.test(file))
  .map((file) => readFileSync(file, 'utf8'))
  .join('\n')

if (artifacts.includes('@hozo/core')) {
  throw new Error('Storybook output still imports @hozo/core')
}
if (!artifacts.includes('Hozo Storybook') || !artifacts.includes('hozo-r0-')) {
  throw new Error('Storybook output is missing the compiled Hozo story')
}
if (!artifacts.includes('background-color')) {
  throw new Error('Storybook output is missing Hozo-generated CSS')
}

console.log('Storybook static build check passed')
