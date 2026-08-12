# Dowel

> A Rust-powered universal UI compiler and accessibility-first layer for React Native.

**Status: early-stage / design phase.** Nothing here runs yet — this repository is scaffolding. See [docs/proposal.md](docs/proposal.md) for the full design document.

## What is Dowel

Dowel compiles React Native source toward the platform it actually runs on:

- **Web** — semantic DOM, CSS, ARIA, minimal runtime
- **Native** — React Native / Fabric, minimal runtime

New projects use `@dowel/core` (a thin set of canonical primitives) as the recommended entry point. Existing React Native / React Native for Web projects can adopt `@dowel/compiler` incrementally, without rewriting existing code.

Dowel is not a new UI framework, a styling library, or a replacement for React Native for Web. It's a compilation layer that sits underneath the existing React Native ecosystem — respecting it, not replacing it.

Accessibility is a first-class requirement from v1, not an add-on.

## Architecture

```
                       Application
                           │
             ┌─────────────┴─────────────┐
             │                           │
        @dowel/core                Existing RN code
             │                           │
             └─────────────┬─────────────┘
                           │
                           ▼
                    Dowel Compiler
                      (Rust core)
                           │
             ┌─────────────┼─────────────┐
             │             │             │
          Style IR    Semantic IR    Diagnostics
             │             │             │
             └─────────────┼─────────────┘
                           │
                       Dowel IR
                           │
             ┌─────────────┴─────────────┐
             │                           │
         Web backend                Native backend
             │                           │
        DOM + CSS + ARIA           React Native
        semantic HTML              View / Text
                                     StyleSheet
             │
         fallback
             │
             RNW
```

## Repository layout

```
packages/
  core/            @dowel/core        — canonical primitives for new projects
  compiler/        @dowel/compiler    — JS entry point over the Rust compiler
  runtime/         @dowel/runtime     — genuinely dynamic styles, interaction, a11y behavior
  tailwind/        @dowel/tailwind    — Tailwind → Style IR
  a11y/            @dowel/a11y        — complex accessibility primitives (Dialog, ...)
  vite-plugin/     @dowel/vite-plugin — Web bundler integration

crates/
  dowel_ir/        platform-independent IR shared across the pipeline
  dowel_parser/    TSX analysis + Style IR construction (oxc)
  dowel_web/       Dowel IR -> DOM/CSS/ARIA lowering
  dowel_napi/      Node native binding (napi-rs)

examples/
  login-demo/      Phase 0 benchmark app

docs/
  proposal.md      full design document
```

A `dowel_native` crate and a Metro-based native bundler integration will follow once the Web path (Vite) is validated. See [docs/proposal.md](docs/proposal.md) §13 for the phased roadmap.

## Status

Phase 0 (vertical prototype) has not started. Nothing in `packages/` or `crates/` is implemented yet — this is scaffolding only.

## License

MIT — see [LICENSE](LICENSE).
