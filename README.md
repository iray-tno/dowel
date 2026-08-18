# Hozo

> A Rust-powered universal UI compiler and accessibility-first layer for React Native.

**Status: working prototype.** The Rust compiler, Vite and Metro integrations, Web and Native lowerers, runtime adapters, conformance suite, and example applications are implemented and tested. It is not published or production-stable yet. See [docs/proposal.md](docs/proposal.md) for the design document.

## What is Hozo

Hozo compiles React Native source toward the platform it actually runs on:

- **Web** — semantic DOM, CSS, ARIA, minimal runtime
- **Native** — React Native / Fabric, minimal runtime

Applications import canonical primitives from `@hozo/core`. Hozo then compiles those primitives to semantic DOM and CSS on Web, or React Native components and `StyleSheet` values on Native. Existing React Native applications can adopt it incrementally, but currently need explicit import changes at the migration boundary.

Hozo is a compilation layer rather than a new component framework. Its Web output is designed to remove React Native for Web from compiled paths while preserving React Native-style component and event contracts where practical.

Accessibility is a first-class requirement from v1, not an add-on.

## Architecture

```
                       Application
                           │
             ┌─────────────┴─────────────┐
             │                           │
        @hozo/core                Existing RN code
             │                           │
             └─────────────┬─────────────┘
                           │
                           ▼
                    Hozo Compiler
                      (Rust core)
                           │
             ┌─────────────┼─────────────┐
             │             │             │
          Style IR    Semantic IR    Diagnostics
             │             │             │
             └─────────────┼─────────────┘
                           │
                       Hozo IR
                           │
             ┌─────────────┴─────────────┐
             │                           │
         Web backend                Native backend
             │                           │
        DOM + CSS + ARIA           React Native
        semantic HTML              View / Text
                                     StyleSheet
```

## Repository layout

```
packages/
  core/            @hozo/core        — canonical primitives for new projects
  compiler/        @hozo/compiler    — JS entry point over the Rust compiler
  runtime/         @hozo/runtime     — genuinely dynamic styles, interaction, a11y behavior
  tailwind/        @hozo/tailwind    — Tailwind → Style IR
  a11y/            @hozo/a11y        — complex accessibility primitives (Dialog, ...)
  vite/            @hozo/vite        — Web bundler integration
  metro/           @hozo/metro       — Native bundler integration
  tailwind-conformance/              — Tailwind/Web/Native comparison and render tests

crates/
  hozo_ir/         platform-independent IR shared across the pipeline
  hozo_parser/     TSX analysis + Style IR construction (oxc)
  hozo_web/        Hozo IR -> DOM/CSS/ARIA lowering
  hozo_native/     Hozo IR -> React Native lowering
  hozo_cache/      project-wide candidate scan cache
  hozo_napi/       Node native binding (napi-rs)

examples/
  login-demo/      Vite Web/SSR validation app
  native-demo/     Metro bundle and Native runtime validation app

docs/
  proposal.md      full design document
```

## Status

The repository currently exercises both lowering backends end to end, including production/minified Web and Android Metro bundles. The automated suite covers compiler transforms, Tailwind conformance, accessibility contracts, semantic Web output, Native render behavior, responder/PanResponder compatibility, contextual variants, transitions, and the current Grid subset.

The main work before a release is packaging and developer experience, broader bundler/framework integrations, migration tooling, and physical-device validation. Public APIs and package boundaries may still change.

## License

MIT — see [LICENSE](LICENSE).
