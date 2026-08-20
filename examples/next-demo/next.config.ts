import path from 'node:path'

import { withHozo } from '@hozo/next'

export default withHozo(
  {
    // The workspace root, not this directory. In a pnpm workspace `next`
    // itself is a symlink into the root's `node_modules/.pnpm`, and
    // Turbopack refuses to read anything above the root it is given.
    turbopack: { root: path.resolve(import.meta.dirname, '../..') },
  },
  { css: 'src/theme.css' },
)
