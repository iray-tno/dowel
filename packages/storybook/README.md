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
