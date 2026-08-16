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

export interface PressableProps {
  className?: string
  children?: ReactNode
  onPress?: MouseEventHandler<HTMLDivElement>
  accessibilityRole?: 'button' | 'link'
}

// No native HTML element matches Pressable's semantics (proposal §10.2):
// without an explicit `accessibilityRole`, this is exactly the
// interactive-without-role case Dowel's compiler is meant to diagnose.
export function Pressable({ className, children, onPress, accessibilityRole }: PressableProps) {
  return (
    <div
      className={className}
      role={accessibilityRole}
      tabIndex={onPress ? 0 : undefined}
      onClick={onPress}
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
}

export function Button({ className, children, onPress, disabled }: ButtonProps) {
  return (
    <button className={className} disabled={disabled} onClick={onPress}>
      {children}
    </button>
  )
}
cargo test 2>&1 | grep -E "^---- |panicked|^error" | head -5

export { TextInput, type TextInputProps } from './text-input.tsx'
