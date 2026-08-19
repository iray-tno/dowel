// Metro configuration for the example, and the shape a real project copies.
//
// Two pieces. The transformer rewrites each `.tsx` before Babel sees it --
// the same division `@hozo/vite` uses on Web, where it runs
// `enforce: 'pre'` ahead of the React plugin. And the candidate module is
// generated at config time: a `className` the compiler can't read at build
// time is resolved on device from a project-wide map, and that map has to
// exist before the bundle starts.

const path = require('node:path')
const { getDefaultConfig } = require('@react-native/metro-config')
const { withHozo } = require('@hozo/metro/config')

const projectRoot = __dirname
const workspaceRoot = path.resolve(projectRoot, '..', '..')



const config = getDefaultConfig(projectRoot)

// A pnpm workspace keeps dependencies outside the project directory, so
// Metro has to be told to look there. Nothing Hozo-specific -- every
// monorepo needs it.
config.watchFolders = [workspaceRoot]
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(workspaceRoot, 'node_modules'),
]

// Exported as a promise, which Metro awaits. withHozo generates the
// candidate module, preserves the rest of this config, and installs its
// transformer while retaining any transformer already configured by RN,
// Expo, or another tool as the upstream handoff.
// `sources` is narrowed to `@hozo/core` here, and this example is the one
// place that should be. Hozo compiles `react-native` imports by default --
// that is how an existing Expo app gets the compiler without a rewrite --
// but `NativeBench.tsx` exists to be the hand-written React Native the
// bundle benchmark measures Hozo *against*. Compiled, it stops being a
// baseline, and the increment silently becomes Hozo-versus-Hozo.
module.exports = withHozo(config, {
  root: projectRoot,
  css: 'global.css',
  sources: ['@hozo/core'],
})
