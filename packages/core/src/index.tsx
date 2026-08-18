// Real, working fallback implementations of Dowel's canonical primitives
// (proposal §2.3: "fall back gracefully" -- these run as plain React
// components whenever the Dowel compiler doesn't (or can't yet) fully
// lower a given usage, not just when it's totally absent). The compiler's
// job is to make invoking these at runtime unnecessary where it can, not
// to make them required.

import { useEffect, useRef, useState, type MouseEventHandler, type ReactNode, type UIEventHandler } from 'react'

export interface DowelLayoutRectangle {
  x: number
  y: number
  width: number
  height: number
}

export interface DowelLayoutEvent {
  nativeEvent: { layout: DowelLayoutRectangle }
}

export interface UniversalProps {
  testID?: string
  nativeID?: string
  pointerEvents?: 'auto' | 'none' | 'box-none' | 'box-only'
  accessibilityState?: {
    disabled?: boolean
    selected?: boolean
    checked?: boolean | 'mixed'
    busy?: boolean
    expanded?: boolean
  }
  accessibilityValue?: { min?: number; max?: number; now?: number; text?: string }
  accessibilityLiveRegion?: 'none' | 'polite' | 'assertive'
  accessibilityLabel?: string
  accessibilityHint?: string
  onLayout?: (event: DowelLayoutEvent) => void
}

function universalDomProps(props: UniversalProps) {
  const state = props.accessibilityState
  const value = props.accessibilityValue
  return {
    'data-testid': props.testID,
    id: props.nativeID,
    'data-dowel-pointer-events': props.pointerEvents,
    'aria-disabled': state?.disabled,
    'aria-selected': state?.selected,
    'aria-checked': state?.checked,
    'aria-busy': state?.busy,
    'aria-expanded': state?.expanded,
    'aria-valuemin': value?.min,
    'aria-valuemax': value?.max,
    'aria-valuenow': value?.now,
    'aria-valuetext': value?.text,
    'aria-live': props.accessibilityLiveRegion === 'none' ? undefined : props.accessibilityLiveRegion,
    'aria-label': props.accessibilityLabel,
    'aria-description': props.accessibilityHint,
  } as const
}

function useLayoutRef<T extends HTMLElement>(onLayout?: (event: DowelLayoutEvent) => void) {
  const elementRef = useRef<T>(null)
  const callbackRef = useRef(onLayout)
  callbackRef.current = onLayout

  useEffect(() => {
    const element = elementRef.current
    if (!element || !callbackRef.current) return

    let previous = ''
    const emit = () => {
      const rect = element.getBoundingClientRect()
      const layout = { x: element.offsetLeft, y: element.offsetTop, width: rect.width, height: rect.height }
      const key = `${layout.x}:${layout.y}:${layout.width}:${layout.height}`
      if (key === previous) return
      previous = key
      callbackRef.current?.({ nativeEvent: { layout } })
    }

    emit()
    if (typeof ResizeObserver === 'undefined') return
    const observer = new ResizeObserver(emit)
    observer.observe(element)
    return () => observer.disconnect()
  }, [Boolean(onLayout)])

  return elementRef
}

function useScrollHandler<T extends HTMLElement>(
  onScroll?: (event: DowelScrollEvent) => void,
  scrollEventThrottle = 0,
): UIEventHandler<T> | undefined {
  const lastEmission = useRef(0)
  if (!onScroll) return undefined
  return (event) => {
    const now = Date.now()
    if (scrollEventThrottle > 0 && now - lastEmission.current < scrollEventThrottle) return
    lastEmission.current = now
    const target = event.currentTarget
    onScroll({
      nativeEvent: {
        contentOffset: { x: target.scrollLeft, y: target.scrollTop },
        contentSize: { width: target.scrollWidth, height: target.scrollHeight },
        layoutMeasurement: { width: target.clientWidth, height: target.clientHeight },
      },
    })
  }
}

export interface ViewProps extends UniversalProps {
  className?: string
  children?: ReactNode
}

export function View({ className, children, onLayout, ...universal }: ViewProps) {
  const ref = useLayoutRef<HTMLDivElement>(onLayout)
  return <div ref={ref} className={className} {...universalDomProps(universal)}>{children}</div>
}

export interface TextProps extends UniversalProps {
  className?: string
  children?: ReactNode
}

export function Text({ className, children, onLayout, ...universal }: TextProps) {
  const ref = useLayoutRef<HTMLSpanElement>(onLayout)
  return <span ref={ref} className={className} {...universalDomProps(universal)}>{children}</span>
}

export type SemanticTextProps = TextProps

export function Paragraph({ className, children, onLayout, ...universal }: SemanticTextProps) {
  const ref = useLayoutRef<HTMLParagraphElement>(onLayout)
  return <p ref={ref} className={className} {...universalDomProps(universal)}>{children}</p>
}

export interface HeadingProps extends SemanticTextProps {
  level?: 1 | 2 | 3 | 4 | 5 | 6
}

export function Heading({ level = 1, className, children, onLayout, ...universal }: HeadingProps) {
  const ref = useLayoutRef<HTMLHeadingElement>(onLayout)
  const Tag = `h${level}` as 'h1' | 'h2' | 'h3' | 'h4' | 'h5' | 'h6'
  return <Tag ref={ref} className={className} {...universalDomProps(universal)}>{children}</Tag>
}

export function Section({ className, children, onLayout, ...universal }: ViewProps) {
  const ref = useLayoutRef<HTMLElement>(onLayout)
  return <section ref={ref} className={className} {...universalDomProps(universal)}>{children}</section>
}

export interface ImageProps extends UniversalProps {
  className?: string
  /** URL/import on Web; URI metadata or Metro's numeric asset id on Native. */
  src: DowelImageSource
  /** Native loading placeholder; Web uses it when the primary source fails or cannot be resolved. */
  defaultSource?: DowelImageSource
  /** Empty string marks a decorative image. */
  alt?: string
  accessibilityLabel?: string
  onLoad?: (event: unknown) => void
  onError?: (event: unknown) => void
}

export interface DowelImageSourceObject {
  uri?: string
  /** ESM namespace shape returned by some asset bundlers. */
  default?: string
}

export type DowelImageSource = string | number | DowelImageSourceObject | readonly DowelImageSourceObject[]

function webImageSource(source?: DowelImageSource): string | undefined {
  if (typeof source === 'string') return source
  if (!source || typeof source !== 'object') return undefined
  if (Array.isArray(source)) {
    for (const candidate of source) {
      const resolved = webImageSource(candidate)
      if (resolved) return resolved
    }
    return undefined
  }
  const object = source as DowelImageSourceObject
  return typeof object.uri === 'string' ? object.uri : typeof object.default === 'string' ? object.default : undefined
}

export function Image({ className, src, defaultSource, alt, accessibilityLabel, onLoad, onError, onLayout, ...universal }: ImageProps) {
  const ref = useLayoutRef<HTMLImageElement>(onLayout)
  const [failed, setFailed] = useState(false)
  useEffect(() => setFailed(false), [src])
  const webSrc = (failed ? undefined : webImageSource(src)) ?? webImageSource(defaultSource)
  return (
    <img
      ref={ref}
      className={className}
      src={webSrc}
      alt={alt ?? accessibilityLabel ?? ''}
      onLoad={onLoad}
      onError={(event) => {
        setFailed(true)
        onError?.(event)
      }}
      {...universalDomProps(universal)}
    />
  )
}

export interface ScrollViewProps extends UniversalProps {
  className?: string
  children?: ReactNode
  horizontal?: boolean
  refreshing?: boolean
  onRefresh?: () => void
  keyboardShouldPersistTaps?: 'always' | 'never' | 'handled'
  showsVerticalScrollIndicator?: boolean
  showsHorizontalScrollIndicator?: boolean
  accessibilityLabel?: string
  accessibilityHint?: string
  onScroll?: (event: DowelScrollEvent) => void
  scrollEventThrottle?: number
}

export interface DowelScrollEvent {
  nativeEvent: {
    contentOffset: { x: number; y: number }
    contentSize: { width: number; height: number }
    layoutMeasurement: { width: number; height: number }
  }
}

export function ScrollView({
  className,
  children,
  horizontal,
  refreshing,
  onRefresh,
  keyboardShouldPersistTaps: _keyboardShouldPersistTaps,
  showsVerticalScrollIndicator = true,
  showsHorizontalScrollIndicator = true,
  accessibilityLabel,
  accessibilityHint,
  onScroll,
  scrollEventThrottle,
  onLayout,
  ...universal
}: ScrollViewProps) {
  const containerRef = useLayoutRef<HTMLDivElement>(onLayout)
  const handleScroll = useScrollHandler<HTMLDivElement>(onScroll, scrollEventThrottle)
  const showIndicator = horizontal ? showsHorizontalScrollIndicator : showsVerticalScrollIndicator
  return (
    <div
      ref={containerRef}
      className={className}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
      aria-busy={refreshing || undefined}
      onScroll={handleScroll}
      {...universalDomProps(universal)}
      style={horizontal
        ? { overflowX: 'auto', overflowY: 'hidden', scrollbarWidth: showIndicator ? 'auto' : 'none' }
        : { overflowX: 'hidden', overflowY: 'auto', scrollbarWidth: showIndicator ? 'auto' : 'none' }}
    >
      {onRefresh ? (
        <button type="button" onClick={onRefresh} disabled={refreshing}>
          {refreshing ? 'Refreshing…' : 'Refresh'}
        </button>
      ) : null}
      {children}
    </div>
  )
}

export interface FlatListRenderInfo<T> {
  item: T
  index: number
}

export interface FlatListProps<T> extends UniversalProps {
  className?: string
  data: readonly T[]
  renderItem: (info: FlatListRenderInfo<T>) => ReactNode
  keyExtractor?: (item: T, index: number) => string
  ListHeaderComponent?: ReactNode
  ListFooterComponent?: ReactNode
  ListEmptyComponent?: ReactNode
  accessibilityLabel?: string
  accessibilityHint?: string
  horizontal?: boolean
  numColumns?: number
  refreshing?: boolean
  onRefresh?: () => void
  onEndReached?: (info: { distanceFromEnd: number }) => void
  onEndReachedThreshold?: number
  keyboardShouldPersistTaps?: 'always' | 'never' | 'handled'
  showsVerticalScrollIndicator?: boolean
  showsHorizontalScrollIndicator?: boolean
  onScroll?: (event: DowelScrollEvent) => void
  scrollEventThrottle?: number
}

/** Web fallback; Native compilation replaces this with the virtualized RN FlatList. */
export function FlatList<T>({
  className,
  data,
  renderItem,
  keyExtractor,
  ListHeaderComponent,
  ListFooterComponent,
  ListEmptyComponent,
  accessibilityLabel,
  accessibilityHint,
  horizontal,
  numColumns = 1,
  refreshing,
  onRefresh,
  onEndReached,
  onEndReachedThreshold = 0,
  keyboardShouldPersistTaps: _keyboardShouldPersistTaps,
  showsVerticalScrollIndicator = true,
  showsHorizontalScrollIndicator = true,
  onScroll,
  scrollEventThrottle,
  onLayout,
  ...universal
}: FlatListProps<T>) {
  const containerRef = useLayoutRef<HTMLDivElement>(onLayout)
  const endRef = useRef<HTMLDivElement>(null)
  const handleScroll = useScrollHandler<HTMLDivElement>(onScroll, scrollEventThrottle)
  const showIndicator = horizontal ? showsHorizontalScrollIndicator : showsVerticalScrollIndicator

  useEffect(() => {
    const root = containerRef.current
    const target = endRef.current
    if (!onEndReached || data.length === 0 || !root || !target || typeof IntersectionObserver === 'undefined') {
      return
    }
    let fired = false
    const margin = `${Math.max(0, onEndReachedThreshold) * 100}%`
    const observer = new IntersectionObserver(([entry]) => {
      if (entry?.isIntersecting && !fired) {
        fired = true
        observer.disconnect()
        onEndReached({ distanceFromEnd: 0 })
      }
    }, { root, rootMargin: horizontal ? `0px ${margin} 0px 0px` : `0px 0px ${margin} 0px` })
    observer.observe(target)
    return () => observer.disconnect()
  }, [data.length, horizontal, onEndReached, onEndReachedThreshold])

  return (
    <div
      ref={containerRef}
      className={className}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
      aria-busy={refreshing || undefined}
      onScroll={handleScroll}
      {...universalDomProps(universal)}
      style={horizontal
        ? { overflowX: 'auto', overflowY: 'hidden', scrollbarWidth: showIndicator ? 'auto' : 'none' }
        : { overflowX: 'hidden', overflowY: 'auto', scrollbarWidth: showIndicator ? 'auto' : 'none' }}
    >
      {onRefresh ? (
        <button type="button" onClick={onRefresh} disabled={refreshing}>
          {refreshing ? 'Refreshing…' : 'Refresh'}
        </button>
      ) : null}
      {ListHeaderComponent}
      {data.length === 0 ? ListEmptyComponent : null}
      {data.length > 0 ? (
        <div
          role="list"
          style={numColumns > 1
            ? { display: 'grid', gridTemplateColumns: `repeat(${numColumns}, minmax(0, 1fr))` }
            : horizontal ? { display: 'flex', flexDirection: 'row' } : undefined}
        >
          {data.map((item, index) => (
            <div key={keyExtractor?.(item, index) ?? index} role="listitem">
              {renderItem({ item, index })}
            </div>
          ))}
        </div>
      ) : null}
      {ListFooterComponent}
      <div ref={endRef} aria-hidden="true" />
    </div>
  )
}

export interface PressableProps {
  className?: string
  children?: ReactNode
  onPress?: MouseEventHandler<HTMLDivElement>
  accessibilityRole?: 'button' | 'link'
  accessibilityLabel?: string
  accessibilityHint?: string
  disabled?: boolean
}

// No native HTML element matches Pressable's semantics (proposal §10.2):
// without an explicit `accessibilityRole`, this is exactly the
// interactive-without-role case Dowel's compiler is meant to diagnose.
export function Pressable({
  className,
  children,
  onPress,
  accessibilityRole,
  accessibilityLabel,
  accessibilityHint,
  disabled,
}: PressableProps) {
  return (
    <div
      className={className}
      role={accessibilityRole}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
      aria-disabled={disabled || undefined}
      tabIndex={onPress && !disabled ? 0 : undefined}
      onClick={disabled ? undefined : onPress}
    >
      {children}
    </div>
  )
}

export interface ButtonProps {
  className?: string
  children?: ReactNode
  onPress?: MouseEventHandler<HTMLButtonElement>
  disabled?: boolean
  accessibilityLabel?: string
  accessibilityHint?: string
}

export function Button({
  className,
  children,
  onPress,
  disabled,
  accessibilityLabel,
  accessibilityHint,
}: ButtonProps) {
  return (
    <button
      className={className}
      disabled={disabled}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
      onClick={onPress}
    >
      {children}
    </button>
  )
}

export interface LinkProps {
  className?: string
  children?: ReactNode
  href: string
  onPress?: MouseEventHandler<HTMLAnchorElement>
  accessibilityLabel?: string
  accessibilityHint?: string
}

export function Link({
  className,
  children,
  href,
  onPress,
  accessibilityLabel,
  accessibilityHint,
}: LinkProps) {
  return (
    <a
      className={className}
      href={href}
      onClick={onPress}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
    >
      {children}
    </a>
  )
}

export { TextInput, type TextInputProps } from './text-input.tsx'

// Dialog is not implemented here: its behaviour is the whole point of it
// (proposal §10.3), and that lives in `@dowel/a11y` so there is exactly one
// implementation for the compiler to lower to and for tests to cover.
export { DowelDialog as Dialog, type DowelDialogProps as DialogProps } from '@dowel/a11y'
