// Real, working fallback implementations of Hozo's canonical primitives
// (proposal §2.3: "fall back gracefully" -- these run as plain React
// components whenever the Hozo compiler doesn't (or can't yet) fully
// lower a given usage, not just when it's totally absent). The compiler's
// job is to make invoking these at runtime unnecessary where it can, not
// to make them required.

import { useEffect, useRef, useState, type MouseEventHandler, type ReactNode, type UIEventHandler } from 'react'
import { hozoInteractive } from '@hozo/runtime'

import { useResponderDomProps, type ResponderProps } from './responder.ts'

export type {
  HozoResponderEvent,
  HozoResponderTouch,
  HozoTouchHistory,
  HozoTouchTrack,
  ResponderProps,
} from './responder.ts'
export { PanResponder } from './pan-responder.ts'
export type { PanResponderCallbacks, PanResponderGestureState, PanResponderInstance } from './pan-responder.ts'

export interface HozoLayoutRectangle {
  x: number
  y: number
  width: number
  height: number
}

export interface HozoLayoutEvent {
  nativeEvent: { layout: HozoLayoutRectangle }
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
  onLayout?: (event: HozoLayoutEvent) => void
}

/**
 * The universal props every primitive accepts, as DOM attributes.
 *
 * Includes the `data-hozo-*` attributes the generated CSS matches on, and
 * that is a contract rather than an implementation detail: `disabled:` and
 * the rest compile to `[data-hozo-…]` selectors, so a primitive rendering
 * through this file has to carry them or the styles simply come off in the
 * fallback path while working in the compiled one. Which they did --
 * `<View accessibilityState={{ disabled }}>` announced the state and
 * dropped the hook.
 *
 * Spread this *before* any attribute a component writes explicitly. Every
 * key here is named unconditionally, so a prop the component destructured
 * out of `universal` arrives as `undefined` rather than absent -- and a
 * later spread of `undefined` erases what came before it.
 *
 * ScrollView and FlatList had it the other way round, and so rendered
 * without the `aria-label`, `aria-description` and `aria-busy` they were
 * explicitly given. Nothing caught it because nothing type-checked; the
 * first `tsc` run over this package reported all six as
 * "specified more than once, so this usage will be overwritten".
 */
function universalDomProps(props: UniversalProps) {
  const state = props.accessibilityState
  const value = props.accessibilityValue
  return {
    'data-testid': props.testID,
    id: props.nativeID,
    'data-hozo-pointer-events': props.pointerEvents,
    // A presence attribute: React renders `data-x={false}` as the string
    // "false", and `[data-hozo-disabled]` matches that.
    'data-hozo-disabled': state?.disabled ? '' : undefined,
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

function useLayoutRef<T extends HTMLElement>(onLayout?: (event: HozoLayoutEvent) => void) {
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
  onScroll?: (event: HozoScrollEvent) => void,
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

export interface ViewProps extends UniversalProps, ResponderProps {
  className?: string
  children?: ReactNode
}

export function View({ className, children, onLayout, ...universal }: ViewProps) {
  const ref = useLayoutRef<HTMLDivElement>(onLayout)
  const responder = useResponderDomProps(ref, universal)
  return <div ref={ref} className={className} {...universalDomProps(universal)} {...responder}>{children}</div>
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

export function Article({ className, children, onLayout, ...universal }: ViewProps) {
  const ref = useLayoutRef<HTMLElement>(onLayout)
  return <article ref={ref} className={className} {...universalDomProps(universal)}>{children}</article>
}

export function Nav({ className, children, onLayout, ...universal }: ViewProps) {
  const ref = useLayoutRef<HTMLElement>(onLayout)
  return <nav ref={ref} className={className} {...universalDomProps(universal)}>{children}</nav>
}

export interface ListProps extends ViewProps {
  ordered?: boolean
}

export function List({ ordered, className, children, onLayout, ...universal }: ListProps) {
  // Both hooks are unconditional; only the selected element receives its
  // ref. Keeping the concrete element types avoids weakening the public
  // fallback just to satisfy a polymorphic ref union.
  const orderedRef = useLayoutRef<HTMLOListElement>(onLayout)
  const unorderedRef = useLayoutRef<HTMLUListElement>(onLayout)
  return ordered
    ? <ol ref={orderedRef} className={className} {...universalDomProps(universal)}>{children}</ol>
    : <ul ref={unorderedRef} className={className} {...universalDomProps(universal)}>{children}</ul>
}

export function ListItem({ className, children, onLayout, ...universal }: ViewProps) {
  const ref = useLayoutRef<HTMLLIElement>(onLayout)
  return <li ref={ref} className={className} {...universalDomProps(universal)}>{children}</li>
}

export interface ImageProps extends UniversalProps {
  className?: string
  /** URL/import on Web; URI metadata or Metro's numeric asset id on Native. */
  src: HozoImageSource
  /** Native loading placeholder; Web uses it when the primary source fails or cannot be resolved. */
  defaultSource?: HozoImageSource
  /** Empty string marks a decorative image. */
  alt?: string
  accessibilityLabel?: string
  onLoad?: (event: unknown) => void
  onError?: (event: unknown) => void
}

export interface HozoImageSourceObject {
  uri?: string
  /** ESM namespace shape returned by some asset bundlers. */
  default?: string
}

export type HozoImageSource = string | number | HozoImageSourceObject | readonly HozoImageSourceObject[]

function webImageSource(source?: HozoImageSource): string | undefined {
  if (typeof source === 'string') return source
  if (!source || typeof source !== 'object') return undefined
  if (Array.isArray(source)) {
    for (const candidate of source) {
      const resolved = webImageSource(candidate)
      if (resolved) return resolved
    }
    return undefined
  }
  const object = source as HozoImageSourceObject
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
  onScroll?: (event: HozoScrollEvent) => void
  scrollEventThrottle?: number
}

export interface HozoScrollEvent {
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
      // Spread before the three below rather than after: see the note on
      // `universalDomProps`.
      {...universalDomProps(universal)}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
      aria-busy={refreshing || undefined}
      onScroll={handleScroll}
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
  onScroll?: (event: HozoScrollEvent) => void
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
      // Spread before the three below rather than after: see the note on
      // `universalDomProps`.
      {...universalDomProps(universal)}
      aria-label={accessibilityLabel}
      aria-description={accessibilityHint}
      aria-busy={refreshing || undefined}
      onScroll={handleScroll}
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

export interface PressableProps extends UniversalProps, ResponderProps {
  className?: string
  children?: ReactNode
  onPress?: MouseEventHandler<HTMLDivElement>
  accessibilityRole?: 'button' | 'link'
  disabled?: boolean
}

// No native HTML element matches Pressable's semantics (proposal §10.2):
// without an explicit `accessibilityRole`, this is exactly the
// interactive-without-role case Hozo's compiler is meant to diagnose.
export function Pressable({
  className,
  children,
  onPress,
  accessibilityRole,
  disabled,
  onLayout,
  ...universal
}: PressableProps) {
  const ref = useLayoutRef<HTMLDivElement>(onLayout)
  const responder = useResponderDomProps(ref, universal, !disabled)
  // Both spellings of the state, folded the way React Native folds them --
  // `Pressable.js` merges the `disabled` prop into `accessibilityState`,
  // and the compiled path merges them into one guard. Two sources for one
  // attribute is how they end up disagreeing.
  const isDisabled = disabled || universal.accessibilityState?.disabled
  // Without an `onPress` this is not a control, so it gets no tab stop and
  // no key handlers -- only the announcement, which a disabled region is
  // still entitled to.
  const interaction = onPress
    ? hozoInteractive(onPress, isDisabled)
    : { 'aria-disabled': isDisabled || undefined }
  return (
    <div
      ref={ref}
      className={className}
      // Spread before anything written explicitly below, for the reason on
      // `universalDomProps` -- it names every key unconditionally, so a
      // later spread of `undefined` erases what came before it.
      {...universalDomProps(universal)}
      role={accessibilityRole}
      // The same call the compiled path makes, so the two cannot answer
      // differently. Everything `disabled` means is in there: see
      // `@hozo/runtime`'s `interactive.ts` and docs/decisions/001.
      //
      // Written out here, this had already drifted twice -- it suppressed
      // the click but not the keyboard, and the compiled path suppressed
      // neither.
      {...interaction}
      {...responder}
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
      // A `<button>` in a `<form>` defaults to `type="submit"`, and React
      // Native has no forms -- so this submitted whatever it happened to
      // be inside, on top of calling `onPress`. The compiled path emits
      // the same attribute.
      type="button"
      className={className}
      disabled={disabled}
      // `disabled:` compiles to `[data-hozo-disabled]` so that one
      // selector works on every element -- `:disabled` matches form
      // controls only, and a Pressable is a `<div>`. A real `<button>`
      // still needs the attribute, or the rule stops matching here while
      // it matches everywhere else.
      data-hozo-disabled={disabled ? '' : undefined}
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
// (proposal §10.3), and that lives in `@hozo/a11y` so there is exactly one
// implementation for the compiler to lower to and for tests to cover.
export { HozoDialog as Dialog, type HozoDialogProps as DialogProps } from '@hozo/a11y'
