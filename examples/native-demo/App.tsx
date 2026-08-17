// The Native example, and the first Dowel source ever put through a real
// Metro bundle. It deliberately exercises the paths that only exist on this
// backend -- the ones a Web-only example would never reach.

import { View, Text, Pressable, TextInput, Dialog } from '@dowel/core'
import { useState } from 'react'

export default function App() {
  const [email, setEmail] = useState('')
  const [confirming, setConfirming] = useState(false)

  return (
    // `text-*` on a View: React Native inherits text styles only from a
    // Text, so the compiler carries these down to the Texts below.
    <View className="flex-1 p-6 gap-4 bg-slate-50 text-slate-900">
      <Text className="text-2xl font-bold">Sign in</Text>

      <TextInput
        className="rounded-lg border border-slate-300 p-3 placeholder-slate-400"
        accessibilityLabel="Email address"
        placeholder="you@example.com"
        value={email}
        onChangeText={setEmail}
      />

      {/* `space-y-*` is the runtime component: which child is last isn't
          knowable at build time once one of them is an expression. */}
      <View className="space-y-2">
        <Text className="text-sm">We never share your address.</Text>
        <Text className="text-sm">You can delete it at any time.</Text>
      </View>

      <Pressable
        className="rounded-lg bg-brand p-3 hover:opacity-90 focus:opacity-75"
        accessibilityRole="button"
        onPress={() => setConfirming(true)}
      >
        <Text className="text-center font-bold text-white">Continue</Text>
      </Pressable>

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
