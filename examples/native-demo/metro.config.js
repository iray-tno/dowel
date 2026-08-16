// Metro configuration for the example, and the shape a real project copies.
//
// Two pieces. The transformer rewrites each `.tsx` before Babel sees it --
// the same division `@dowel/vite-plugin` uses on Web, where it runs
// `enforce: 'pre'` ahead of the React plugin. And the candidate module is
// generated at config time: a `className` the compiler can't read at build
// time is resolved on device from a project-wide map, and that map has to
// exist before the bundle starts.

const path = require('node:path')
const { getDefaultConfig } = require('@react-native/metro-config')
const { generateCandidateModule } = require('@dowel/metro-transformer/project')

const projectRoot = __dirname
const workspaceRoot = path.resolve(projectRoot, '..', '..')

generateCandidateModule(projectRoot)

const config = getDefaultConfig(projectRoot)

config.transformer.babelTransformerPath = require.resolve('@dowel/metro-transformer')

// A pnpm workspace keeps dependencies outside the project directory, so
// Metro has to be told to look there. Nothing Dowel-specific -- every
// monorepo needs it.
config.watchFolders = [workspaceRoot]
config.resolver.nodeModulesPaths = [
  path.resolve(projectRoot, 'node_modules'),
  path.resolve(workspaceRoot, 'node_modules'),
]

module.exports = config
