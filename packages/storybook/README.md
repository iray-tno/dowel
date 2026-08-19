# @hozo/storybook

Add the preset to `.storybook/main.ts`; no custom `viteFinal` is needed:

```ts
export default {
  framework: '@storybook/react-vite',
  addons: ['@hozo/storybook'],
}
```

For a non-standard Tailwind entry, pass the same options as `@hozo/vite`:

```ts
addons: [{ name: '@hozo/storybook', options: { css: 'src/theme.css' } }]
```

Hozo runs before Storybook's React transforms and lowers `@hozo/core` directly to semantic Web output, so the ordinary React Vite framework does not require React Native for Web.

## Dev mode

`storybook dev` runs Vite, and the preset installs the same plugin the
Vite integration uses, so HMR works the way it does there. Verified
against a running server rather than assumed: editing a `className`
updates the component and its stylesheet without a restart.

One characteristic worth knowing, because it is visible: a style-only edit
reaches the browser in two HMR rounds. The companion stylesheet is written
*during* the source module's transform, so the `.tsx` change triggers the
first round, that transform writes the CSS, and the watcher seeing the new
CSS triggers the second.
