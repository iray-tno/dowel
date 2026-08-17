// Real, working fallback implementations of Dowel's canonical primitives
// (proposal §2.3: "fall back gracefully" -- these run as plain React
// components whenever the Dowel compiler doesn't (or can't yet) fully
// lower a given usage, not just when it's totally absent). The compiler's
// job is to make invoking these at runtime unnecessary where it can, not
// to make them required.

import type { MouseEventHandler, ReactNode } from 'react'

export interface ViewProps {
  className?: string
  children?: ReactNode
}

export function View({ className, children }: ViewProps) {
  return <div className={className}>{children}</div>
}

export interface TextProps {
  className?: string
  children?: ReactNode
}

export function Text({ className, children }: TextProps) {
  return <span className={className}>{children}</span>
}

export interface ImageProps {
  className?: string
  src: string
  /** Empty string marks a decorative image. */
  alt?: string
  accessibilityLabel?: string
}

export function Image({ className, src, alt, accessibilityLabel }: ImageProps) {
  return <img className={className} src={src} alt={alt ?? accessibilityLabel ?? ''} />
}

export interface ScrollViewProps {
  className?: string
  children?: ReactNode
  horizontal?: boolean
}

export function ScrollView({ className, children, horizontal }: ScrollViewProps) {
  return (
    <div
      className={className}
      style={horizontal ? { overflowX: 'auto', overflowY: 'hidden' } : { overflowX: 'hidden', overflowY: 'auto' }}
    >
      {children}
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
