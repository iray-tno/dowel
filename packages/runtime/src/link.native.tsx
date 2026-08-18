import type { ReactNode } from 'react'
import { Linking, Pressable, type PressableProps } from 'react-native'

export interface HozoLinkProps extends Omit<PressableProps, 'onPress'> {
  href: string
  onPress?: PressableProps['onPress']
  children?: ReactNode
}

/** Native semantic link with the same destination-bearing API as `<a>`. */
export function HozoLink({ href, onPress, children, ...props }: HozoLinkProps) {
  return (
    <Pressable
      {...props}
      accessibilityRole="link"
      onPress={(event) => {
        onPress?.(event)
        if (!event.defaultPrevented) void Linking.openURL(href)
      }}
    >
      {children}
    </Pressable>
  )
}
