import { View, Text, Button } from '@hozo/core'
import { accentFor } from './variants'

export function Login({ enabled = true }: { enabled?: boolean }) {
  return (
    <View className="flex-1 items-center justify-center p-6">
      <Text className="text-xl font-bold">Welcome</Text>

      <Button className={`mt-4 px-4 py-2 ${accentFor(enabled)}`}>Continue</Button>
    </View>
  )
}
