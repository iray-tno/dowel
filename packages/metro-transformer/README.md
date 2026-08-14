# @dowel/metro-transformer

Metro integration for Dowel's React Native backend. Lowers `View`/`Text`/`Pressable`/`Button` from `@dowel/core` into real React Native primitives plus a `StyleSheet.create({...})`, rewriting each component in place.

> Not yet verified against a running Metro/Expo build — no device or simulator was available while it was written. The source-rewrite logic (`transformDowelSource`) is unit-tested; the Metro API surface on top of it is not.

## Setup

`metro.config.js`:

```js
const { getDefaultConfig } = require('expo/metro-config') // or '@react-native/metro-config'
const { generateCandidateModule } = require('@dowel/metro-transformer')

const projectRoot = __dirname
generateCandidateModule(projectRoot)

const config = getDefaultConfig(projectRoot)
config.transformer.babelTransformerPath = require.resolve('@dowel/metro-transformer')
module.exports = config
```

Both lines are needed. The transformer alone handles every `className` the compiler can read; `generateCandidateModule` covers the ones it can't.

## What `generateCandidateModule` is for

A `className` Dowel can't read statically — `<View className={getVariant()} />` — still has to produce styles. On Web that's free: the class string reaches the DOM and the browser matches it against a generated stylesheet. React Native has no CSS engine, so the string has to be resolved on device.

`generateCandidateModule` scans the project for class-shaped strings and writes `node_modules/.dowel/candidates.native.js`: a class-name → style-object map plus a resolver bound to it (from `@dowel/runtime`). Files with an unreadable `className` import it; files without one don't.

It runs at config load rather than inside the transformer because Metro transforms in `jest-worker` subprocesses. Scanning there would mean several processes writing one cache file; the config layer is ordinary main-process code, so there's exactly one writer.

**Limitation:** the module is generated once, at config load. A class that only becomes a candidate *after* Metro started — a new string literal in a helper module — needs a Metro restart to appear. (`react-native-css` documents the same restriction for its transformer.)

**Limitation:** only unconditional utilities survive this path. A style object can't express `hover:`, `md:`, or `pressed:`, and making it able to would mean per-component state tracking — a runtime CSS engine, which Dowel deliberately doesn't ship. Those classes are recorded with the reason and warned about *when they're actually used*, not at build time: appearing in the scan doesn't prove any expression ever produces one. Write them as a static `className` and they compile to a real style variant with no runtime involved.

## Errors vs. warnings

Error-severity diagnostics stop the build. The case that exists for is a Web-only utility (`block`, `grid`, `h-screen`) reaching the Native backend: there's no correct output, so continuing would ship a layout that looks right on Web and is silently wrong on device.

Everything else is a warning printed during the build.
