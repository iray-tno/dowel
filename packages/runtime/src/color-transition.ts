function rgba(color: string): [number, number, number, number] | null {
  if (color === 'transparent') return [0, 0, 0, 0]
  const match = /^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(color)
  if (!match) return null
  let hex = match[1]
  if (hex.length === 3) hex = [...hex].map((digit) => digit + digit).join('')
  const alpha = hex.length === 8 ? parseInt(hex.slice(6, 8), 16) / 255 : 1
  return [parseInt(hex.slice(0, 2), 16), parseInt(hex.slice(2, 4), 16), parseInt(hex.slice(4, 6), 16), alpha]
}

/** The visible colour at `progress`, used when an interrupted transition restarts. */
export function blendColor(from: string, to: string, progress: number) {
  const a = rgba(from)
  const b = rgba(to)
  if (!a || !b) return progress < 0.5 ? from : to
  const values = a.map((value, index) => value + (b[index] - value) * progress)
  return `rgba(${Math.round(values[0])}, ${Math.round(values[1])}, ${Math.round(values[2])}, ${values[3]})`
}
