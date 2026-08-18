import { FlatList, Text, View } from '@hozo/core'

const rows = ['one', 'two', 'three']

export default function HozoBench() {
  return (
    <View className="flex-1 bg-white p-4">
      <Text className="mb-2 text-xl font-bold">Bundle benchmark</Text>
      <FlatList
        data={rows}
        keyExtractor={(item) => item}
        renderItem={({ item }) => <Text className="p-2">{item}</Text>}
      />
    </View>
  )
}
