// Every platform Hozo ships a compiled binding for, as one table.
//
// The risk this exists to remove: the loader computes a package name at
// *runtime* from `process.platform`/`process.arch`, and the packer
// produces package names at *build* time from a Rust target triple. Those
// are two lists that must agree exactly, and a disagreement is invisible
// until a user on the odd platform installs a published package and is
// told no native addon could be loaded for it. `native-targets.test.ts`
// walks the table through both.
//
// The npm names are napi-rs's convention, which is worth following even
// though nothing here uses its CLI: it is what `os`/`cpu`/`libc` fields
// and every prebuild tutorial assume, so a contributor reading this
// recognises the shape.

export interface NativeTarget {
  /** Rust target triple passed to `cargo build --target`. */
  triple: string
  /** npm package name, without the version. */
  packageName: string
  /** `process.platform` this serves. */
  platform: NodeJS.Platform
  /** `process.arch` this serves. */
  arch: string
  /** Which C library, on the platform where that is a question. */
  libc?: 'gnu' | 'musl'
  /** The GitHub Actions runner that can build it. */
  runner: string
}

export const NATIVE_TARGETS: NativeTarget[] = [
  {
    triple: 'x86_64-pc-windows-msvc',
    packageName: '@hozo/compiler-win32-x64-msvc',
    platform: 'win32',
    arch: 'x64',
    runner: 'windows-latest',
  },
  {
    triple: 'aarch64-pc-windows-msvc',
    packageName: '@hozo/compiler-win32-arm64-msvc',
    platform: 'win32',
    arch: 'arm64',
    runner: 'windows-11-arm',
  },
  {
    triple: 'x86_64-apple-darwin',
    packageName: '@hozo/compiler-darwin-x64',
    platform: 'darwin',
    arch: 'x64',
    runner: 'macos-latest',
  },
  {
    triple: 'aarch64-apple-darwin',
    packageName: '@hozo/compiler-darwin-arm64',
    platform: 'darwin',
    arch: 'arm64',
    runner: 'macos-latest',
  },
  {
    triple: 'x86_64-unknown-linux-gnu',
    packageName: '@hozo/compiler-linux-x64-gnu',
    platform: 'linux',
    arch: 'x64',
    libc: 'gnu',
    runner: 'ubuntu-latest',
  },
  {
    triple: 'x86_64-unknown-linux-musl',
    packageName: '@hozo/compiler-linux-x64-musl',
    platform: 'linux',
    arch: 'x64',
    libc: 'musl',
    runner: 'ubuntu-latest',
  },
  {
    triple: 'aarch64-unknown-linux-gnu',
    packageName: '@hozo/compiler-linux-arm64-gnu',
    platform: 'linux',
    arch: 'arm64',
    libc: 'gnu',
    runner: 'ubuntu-24.04-arm',
  },
  {
    triple: 'aarch64-unknown-linux-musl',
    packageName: '@hozo/compiler-linux-arm64-musl',
    platform: 'linux',
    arch: 'arm64',
    libc: 'musl',
    runner: 'ubuntu-24.04-arm',
  },
]

/** The cdylib file extension Cargo produces on a platform. */
export function cdylibExtension(platform: NodeJS.Platform): string | undefined {
  return { win32: 'dll', darwin: 'dylib', linux: 'so' }[platform as 'win32' | 'darwin' | 'linux']
}

/**
 * The cdylib file name Cargo produces.
 *
 * Unix prefixes the crate name with `lib` and Windows doesn't, which is
 * the kind of detail that only shows up on the machine that has it.
 */
export function cdylibFileName(platform: NodeJS.Platform, crate: string): string {
  const extension = cdylibExtension(platform)
  if (!extension) throw new Error(`no cdylib extension known for platform "${platform}"`)
  return platform === 'win32' ? `${crate}.${extension}` : `lib${crate}.${extension}`
}

/** The target this machine builds by default. */
export function hostTarget(
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
  libc: 'gnu' | 'musl' = 'gnu',
): NativeTarget | undefined {
  return NATIVE_TARGETS.find(
    (target) =>
      target.platform === platform &&
      target.arch === arch &&
      (target.libc === undefined || target.libc === libc),
  )
}

/** The manifest `@hozo/compiler` publishes, given the one it develops under. */
export function publishManifest(
  current: Record<string, unknown> & { version: string },
): Record<string, unknown> {
  const optionalDependencies = Object.fromEntries(
    NATIVE_TARGETS.map((target) => [target.packageName, current.version]),
  )
  const { private: _private, ...rest } = current
  return {
    ...rest,
    // Optional is what makes the whole scheme work: npm evaluates each
    // one's `os`/`cpu`/`libc`, installs the single package that matches,
    // and skips the other seven without complaint. Required would mean
    // shipping eight binaries to everyone.
    //
    // Exact versions, not ranges. A binding is compiled from the same
    // source as the JavaScript calling it, and there is no such thing as a
    // compatible-but-different build of it.
    optionalDependencies,
  }
}
