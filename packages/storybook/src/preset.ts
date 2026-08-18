import type { UserConfig } from 'vite'

import { hozo, type HozoOptions } from '@hozo/vite'

export type HozoStorybookOptions = HozoOptions

/** Storybook preset hook: installs Hozo before framework transform plugins. */
export function viteFinal(
  config: UserConfig,
  options: HozoStorybookOptions = {},
): UserConfig {
  const { css, content, debug } = options
  return {
    ...config,
    plugins: [hozo({ css, content, debug }), ...(config.plugins ?? [])],
  }
}
