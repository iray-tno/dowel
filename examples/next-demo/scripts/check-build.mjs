// Proves the Next.js integration did its job, rather than that the build
// exited zero. A Turbopack build succeeds perfectly well with the loader
// never having run.
import { readdirSync, readFileSync } from 'node:fs'
import path from 'node:path'

function filesUnder(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const file = path.join(directory, entry.name)
    return entry.isDirectory() ? filesUnder(file) : [file]
  })
}

const artifacts = filesUnder('.next').filter((file) => /\.(?:js|mjs|css|html)$/.test(file))
const output = artifacts.map((file) => readFileSync(file, 'utf8')).join('\n')
const html = readFileSync(path.join('.next', 'server', 'app', 'index.html'), 'utf8')

const checks = [
  // Lowering: canonical primitives became semantic HTML, and the class is
  // the compiled scoped one rather than the authored utility string.
  [html.includes('<section class="hozo-r0-'), 'Section did not lower to <section>'],
  [html.includes('<h1 class="hozo-r0-'), 'Heading level={1} did not lower to <h1>'],
  [html.includes('<p class="hozo-r0-'), 'Paragraph did not lower to <p>'],
  [html.includes('<button class="hozo-r0-'), 'Button did not lower to <button>'],
  [!/class="[^"]*\bp-8\b/.test(html), 'an authored utility class survived to the DOM'],
  // The compiler's own output reached the browser as CSS.
  [output.includes('.hozo-r0-'), 'no Hozo-generated CSS in the build output'],
  // The project theme was read: `bg-brand` only resolves through
  // src/theme.css, which nothing imports.
  [/background-color:\s*(?:#3082f6|lab\(|oklch\(62%)/.test(output), 'project theme token bg-brand did not resolve'],
  // Variants: `md:hover:` needs both the width query and the capability one.
  [/@media\s*\(min-width:\s*768px\)/.test(output), 'responsive variant produced no media query'],
  [/@media\s*\(hover:\s*hover\)/.test(output), 'hover variant produced no capability query'],
  // Tier three: a class only a runtime expression produces is covered by
  // the project-wide scan, which under Next runs at config time.
  [output.includes('.bg-emerald-500'), 'candidate stylesheet did not reach the build'],
  // Nothing of the authoring layer is left in the bundle.
  [!output.includes('@hozo/core'), 'Next.js output still imports @hozo/core'],
]

for (const [ok, message] of checks) {
  if (!ok) throw new Error(message)
}

console.log(`Next.js build check passed (${artifacts.length} artifacts)`)
