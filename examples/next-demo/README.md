# Hozo + Next.js

```sh
pnpm --filter @hozo/example-next test
```

Builds the App Router app **twice**, once with each of Next 16's bundlers,
and asserts the same things about both. Both are checked because they are
not interchangeable: webpack applies a module's loaders right-to-left, so
the rule Hozo prepends runs last unless it also says `enforce: 'pre'` —
and a Turbopack-only check passed the whole time that was wrong.

`scripts/check-build.mjs` asserts the parts a green build says nothing
about:

- `Section`/`Heading`/`Paragraph`/`Button` became `<section>`/`<h1>`/`<p>`/
  `<button>` in the prerendered HTML, carrying compiled scoped classes
  rather than the utility strings they were written with.
- The generated CSS reached the browser.
- `bg-brand` resolved, which it can only do through `src/theme.css` — a
  file nothing imports, read by Hozo for its tokens.
- `md:hover:` produced both a width query and a `(hover: hover)` one.
- `bg-emerald-500`, which exists only as another module's return value, is
  covered by the project-wide candidate stylesheet.
- `@hozo/core` is gone from the output entirely.
