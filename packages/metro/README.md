# @hozo/metro

Metro integration for Hozo's React Native backend. Lowers `View`/`Text`/`Pressable`/`Button` from `@hozo/core` into real React Native primitives plus a `StyleSheet.create({...})`, rewriting each component in place.

The integration is exercised by development and minified production Metro bundles. Physical-device validation is still pending.

## Setup

`metro.config.js`:

```js
const { getDefaultConfig } = require('expo/metro-config') // or '@react-native/metro-config'
const { withHozo } = require('@hozo/metro/config')

const projectRoot = __dirname
const config = getDefaultConfig(projectRoot)
module.exports = withHozo(config, { projectRoot })
```

Metro accepts a promised config, so `withHozo` can resolve the Tailwind theme and generate the dynamic candidate module before bundling begins. It preserves the supplied config and records an existing Babel transformer as its upstream, including Expo's or another tool's transformer.

## What the generated candidate module is for

A `className` Hozo can't read statically — `<View className={getVariant()} />` — still has to produce styles. On Web that's free: the class string reaches the DOM and the browser matches it against a generated stylesheet. React Native has no CSS engine, so the string has to be resolved on device.

`withHozo` scans the project for class-shaped strings and writes `node_modules/.hozo/candidates.native.js`: a class-name → style-object map plus a resolver bound to it (from `@hozo/runtime`). Files with an unreadable `className` import it; files without one don't. The lower-level `generateCandidateModule` API remains available from `@hozo/metro/project`.

It runs at config load rather than inside the transformer because Metro transforms in `jest-worker` subprocesses. Scanning there would mean several processes writing one cache file; the config layer is ordinary main-process code, so there's exactly one writer.

**Limitation:** the module is generated once, at config load. A class that only becomes a candidate *after* Metro started — a new string literal in a helper module — needs a Metro restart to appear. (`react-native-css` documents the same restriction for its transformer.)

**Limitation:** only unconditional utilities survive *this* path (the runtime-resolved one). A style object can't express `hover:`, `md:`, or `pressed:`, and making it able to would mean per-component state tracking — a runtime CSS engine, which Hozo deliberately doesn't ship. Those classes are recorded with the reason and warned about *when they're actually used*, not at build time: appearing in the scan doesn't prove any expression ever produces one. Write them as a static `className` and they compile to a real style variant with no runtime involved.

## `dark:` and breakpoints need a component function

Written as a static `className`, `dark:` and `sm:`/`md:`/`lg:`/`xl:`/`2xl:` compile to a React hook from `@hozo/runtime`, spliced as a statement at the top of the enclosing component:

```jsx
export function Card() {
  const __hozoDark = useHozoDark()
  return <View style={[styles.hozo_r0_0, __hozoDark && styles.hozo_r0_0_dark]} />
}
```

The hook has to be a statement. Inlining the call into the JSX (`style={[a, useHozoDark() && b]}`) breaks the rules of hooks as soon as the element sits behind a conditional, so JSX at module scope or in a concise arrow body (`() => <View className="dark:..." />`) is a build error naming the fix.

`@hozo/runtime` keeps **one** subscription per app, not one per component, and its snapshot is a coarse value — the breakpoint's name rather than the raw width. A resize that doesn't cross a breakpoint therefore re-renders nothing, and Android's keyboard-driven dimension events never reach it at all, since only width is an input.

## Errors vs. warnings

Error-severity diagnostics stop the build. The case that exists for is a Web-only utility (`block`, `grid`, `h-screen`) reaching the Native backend: there's no correct output, so continuing would ship a layout that looks right on Web and is silently wrong on device.

Everything else is a warning printed during the build.
