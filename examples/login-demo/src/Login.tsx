import { View, Text, Button } from '@dowel/core'

export function Login() {
  return (
    <View className="flex-1 items-center justify-center p-6">
      <Text className="text-xl font-bold">Welcome</Text>

      <Button className="mt-4 px-4 py-2">Continue</Button>
    </View>
  )
}
