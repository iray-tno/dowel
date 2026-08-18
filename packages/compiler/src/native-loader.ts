export type NativeRequire = (specifier: string) => unknown

export interface NativeLoadOptions {
  require: NativeRequire
  localPath: string
  platform?: NodeJS.Platform
  arch?: string
  override?: string
  libc?: 'gnu' | 'musl'
}

export function nativePackageName(
  platform: NodeJS.Platform,
  arch: string,
  libc: 'gnu' | 'musl' = 'gnu',
): string | undefined {
  const target =
    platform === 'win32' && (arch === 'x64' || arch === 'arm64')
      ? `win32-${arch}-msvc`
      : platform === 'darwin' && (arch === 'x64' || arch === 'arm64')
        ? `darwin-${arch}`
        : platform === 'linux' && (arch === 'x64' || arch === 'arm64')
          ? `linux-${arch}-${libc}`
          : undefined
  return target ? `@hozo/compiler-${target}` : undefined
}

function detectLibc(): 'gnu' | 'musl' {
  if (process.platform !== 'linux') return 'gnu'
  const report = process.report?.getReport() as { header?: { glibcVersionRuntime?: string } }
  return report.header?.glibcVersionRuntime ? 'gnu' : 'musl'
}

/** Loads a development addon or the matching future platform package. */
export function loadNativeBinding<T>(options: NativeLoadOptions): T {
  const platform = options.platform ?? process.platform
  const arch = options.arch ?? process.arch
  const override = options.override ?? process.env.HOZO_NATIVE_BINDING
  const platformPackage = nativePackageName(platform, arch, options.libc ?? detectLibc())
  const candidates = override
    ? [override]
    : [options.localPath, platformPackage].filter((candidate): candidate is string => !!candidate)
  const failures: unknown[] = []

  for (const candidate of candidates) {
    try {
      const loaded = options.require(candidate) as { default?: T } | T
      return ((loaded as { default?: T }).default ?? loaded) as T
    } catch (error) {
      failures.push(error)
    }
  }

  const tried = candidates.map((candidate) => `  - ${candidate}`).join('\n') || '  - none'
  const hint = override
    ? `HOZO_NATIVE_BINDING points to ${override}, but it could not be loaded.`
    : platformPackage
      ? `Install @hozo/compiler normally so its optional ${platformPackage} dependency is present.`
      : `Hozo does not yet ship a native compiler for ${platform}/${arch}.`
  throw new Error(
    `@hozo/compiler: no native addon could be loaded for ${platform}/${arch}.\n` +
      `Tried:\n${tried}\n${hint}\n` +
      `Repository contributors can run \`pnpm --filter @hozo/compiler build:native\`.`,
    { cause: new AggregateError(failures, 'Native addon load failures') },
  )
}
