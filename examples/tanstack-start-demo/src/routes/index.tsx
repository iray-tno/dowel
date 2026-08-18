import { Button, Heading, Paragraph, View } from '@hozo/core'
import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/')({ component: Home })

function Home() {
  return (
    <View className="min-h-screen items-center justify-center bg-slate-950 p-8">
      <View className="w-full max-w-xl rounded-2xl bg-white p-8 shadow-xl">
        <Heading level={1} className="text-3xl font-bold text-slate-950">
          Hozo + TanStack Start
        </Heading>
        <Paragraph className="mt-3 text-slate-600">
          Canonical primitives become semantic HTML during SSR and hydration.
        </Paragraph>
        <Button
          accessibilityLabel="Continue with Hozo"
          className="mt-6 rounded-lg bg-blue-600 px-4 py-3 text-white hover:bg-blue-700"
          onPress={() => undefined}
        >
          Continue
        </Button>
      </View>
    </View>
  )
}
