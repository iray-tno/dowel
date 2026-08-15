import { View, Text } from '@dowel/core'
import { accentFor } from './variants'

export function Panel({ show, items }: { show?: boolean; items?: string[] }) {
  return (
    <View className="p-6">
      <Text className="text-xl font-bold">Panel</Text>
      {show && <Text className="text-sm">extra</Text>}
      {(items ?? []).map((i) => (
        <Text key={i} className={accentFor(true)}>{i}</Text>
      ))}
    </View>
  )
}
