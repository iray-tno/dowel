# Hozo + TanStack Start

TanStack Start already uses Vite, so Hozo needs no framework-specific wrapper. Put `hozo()` first so it sees canonical JSX before Start generates its route modules:

```ts
plugins: [hozo(), tanstackStart(), viteReact(), nitro()]
```

`pnpm test` builds the real Start client and server output and verifies that `@hozo/core` was lowered and its CSS was emitted.
