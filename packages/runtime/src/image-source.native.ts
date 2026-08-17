/** Normalizes Dowel's universal Image `src` to React Native's source shape. */
export function dowelImageSource<T extends number | object>(src: string | T): { uri: string } | T {
  return typeof src === 'string' ? { uri: src } : src
}
