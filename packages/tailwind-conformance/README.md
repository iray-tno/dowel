# @dowel/tailwind-conformance

Differential test against the real Tailwind engine. For each candidate utility it compiles the class both ways — through the actual `tailwindcss` package and through Dowel — and compares the results.

```
pnpm --filter @dowel/compiler build:native   # the report needs the native addon
pnpm --filter @dowel/tailwind-conformance report
pnpm --filter @dowel/tailwind-conformance test   # the normalizer's own tests
```

## The two numbers

- **Coverage** — of the candidates Tailwind produces a rule for, how many Dowel emits *something* for. A gap here is unimplemented surface, not a bug.
- **Fidelity** — of those, how many match Tailwind's meaning exactly. A gap here *is* a bug: the class is accepted but compiles to the wrong thing, which is worse than not supporting it.

Both are measured against `src/candidates.ts` — a representative slice of what real app code uses, not all of Tailwind (which is unbounded once arbitrary values count). The denominator is a judgement call, so the number is only meaningful alongside that list.

## Why a normalizer

The two sides constantly differ without disagreeing:

| Tailwind | Dowel |
|---|---|
| `flex: 1` | `flex: 1 1 0%` |
| `padding: calc(var(--spacing) * 4)` | four `padding-*: 16px` longhands |
| `background-color: var(--color-blue-500)` | `background-color: oklch(…)` |
| `line-height: calc(1.75 / 1.25)` | `line-height: 28px` |

So `normalize.ts` resolves custom properties, folds `calc()`, converts rem→px, expands shorthands to longhands, and canonicalizes value spelling — then the two are compared as declaration maps.

**It refuses to guess.** Anything it can't confidently resolve is reported `SKIPPED` rather than counted as a match or a mismatch, because a normalizer that quietly mis-resolves would manufacture both. `normalize.test.ts` pins that behavior directly; it's the load-bearing part of the whole comparison, and if it's wrong every number here is wrong.

## Verdicts

| | meaning |
|---|---|
| `MATCH` | normalized declarations are identical |
| `MISMATCH` | both emit, and they disagree — a fidelity bug |
| `UNSUPPORTED` | Dowel emits nothing — a coverage gap |
| `SKIPPED` | one side couldn't be normalized; no claim made |
