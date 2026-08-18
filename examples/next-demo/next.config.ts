import path from 'node:path'

import { withHozo } from '@hozo/next'

export default withHozo(
  {
    // The workspace root, not this directory. In a pnpm workspace `next`
    // itself is a symlink into the root's `node_modules/.pnpm`, and
    // Turbopack refuses to read anything above the root it is given.
    turbopack: { root: path.resolve(import.meta.dirname, '../..') },
    // `next build` runs `tsc` over everything it can reach, which here
    // means the workspace packages this example is linked to -- raw
    // TypeScript sources that Node's type stripper runs directly and that
    // no tsconfig in this repository type-checks. A project installing
    // published packages would type-check its own files against their
    // `.d.ts` and see none of this. What this example exists to prove is
    // that the build integration works, and that is what the check script
    // asserts.
    typescript: { ignoreBuildErrors: true },
  },
  { css: 'src/theme.css' },
)
