/** Normalizes Hozo's universal Image `src` to React Native's source shape. */
export function hozoImageSource<T extends number | object>(src: string | T): { uri: string } | T {
  return typeof src === 'string' ? { uri: src } : src
}
