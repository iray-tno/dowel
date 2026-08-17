// A bundle fixture and a device acceptance screen. Stable testIDs make
// manual VoiceOver/TalkBack and layout results reproducible.

import { Dialog, FlatList, Image, Pressable, ScrollView, Text, TextInput, View } from '@dowel/core'
import { useState } from 'react'

const rows = [
  { id: 'one', title: 'First virtual row' },
  { id: 'two', title: 'Second virtual row' },
  { id: 'three', title: 'Third virtual row' },
]

export default function App() {
  const [email, setEmail] = useState('')
  const [confirming, setConfirming] = useState(false)
  const [gridWidth, setGridWidth] = useState(0)

  return (
    <View className="flex-1 bg-slate-50 text-slate-900">
      <FlatList
        className="flex-1"
        accessibilityLabel="Dowel native acceptance screen"
        data={rows}
        keyExtractor={(item) => item.id}
        ListHeaderComponent={
          <View className="p-6 space-y-4">
            <Text className="text-2xl font-bold">Dowel device checks</Text>

            <Image
              className="w-20 h-20 rounded-lg object-cover"
              src="https://reactnative.dev/img/tiny_logo.png"
              alt="React Native logo"
              testID="smoke-image"
            />

            <TextInput
              className="rounded-lg border border-slate-300 p-3 placeholder-slate-400"
              accessibilityLabel="Email address"
              accessibilityHint="Enter an address to review in the confirmation dialog"
              placeholder="you@example.com"
              value={email}
              onChangeText={setEmail}
              testID="smoke-input"
            />

            <ScrollView horizontal className="h-20" testID="smoke-horizontal-scroll">
              <View className="flex-row gap-2">
                <View className="w-32 rounded-lg bg-white p-3"><Text>Card one</Text></View>
                <View className="w-32 rounded-lg bg-white p-3"><Text>Card two</Text></View>
                <View className="w-32 rounded-lg bg-white p-3"><Text>Card three</Text></View>
                <View className="w-32 rounded-lg bg-white p-3"><Text>Card four</Text></View>
              </View>
            </ScrollView>

            <View
              className="grid grid-cols-2 gap-2"
              onLayout={({ nativeEvent }) => setGridWidth(Math.round(nativeEvent.layout.width))}
              testID="smoke-grid"
            >
              <View className="row-span-2 rounded-lg bg-white p-3"><Text>Tall</Text></View>
              <View className="rounded-lg bg-white p-3"><Text>Top</Text></View>
              <View className="rounded-lg bg-white p-3"><Text>Bottom</Text></View>
            </View>
            <Text accessibilityLabel={`Measured grid width ${gridWidth}`}>Grid width: {gridWidth}px</Text>

            <Pressable
              className="rounded-lg bg-brand p-3 transition-colors duration-200 hover:bg-blue-700 focus-visible:bg-blue-800"
              accessibilityRole="button"
              accessibilityLabel="Review email address"
              onPress={() => setConfirming(true)}
              testID="smoke-interaction"
            >
              <Text className="text-center font-bold text-white">Continue</Text>
            </Pressable>
          </View>
        }
        renderItem={({ item }) => (
          <View className="mx-6 mb-2 rounded-lg bg-white p-3" testID={`smoke-row-${item.id}`}>
            <Text>{item.title}</Text>
          </View>
        )}
        ListFooterComponent={<View className="h-6" />}
        testID="smoke-list"
      />

      <Dialog
        className="m-6 rounded-xl bg-white p-6"
        open={confirming}
        onClose={() => setConfirming(false)}
        accessibilityLabel="Confirm your address"
      >
        <Text className="text-lg font-bold">Is this right?</Text>
        <Text className="text-slate-600">{email}</Text>
      </Dialog>
    </View>
  )
}
