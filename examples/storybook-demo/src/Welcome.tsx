import { Button, Heading, Paragraph, View } from '@hozo/core'

export function Welcome() {
  return (
    <View className="max-w-lg rounded-2xl bg-white p-8 shadow-xl">
      <Heading level={2} className="text-2xl font-bold text-slate-950">
        Hozo Storybook
      </Heading>
      <Paragraph className="mt-3 text-slate-600">
        This story renders semantic HTML without React Native for Web.
      </Paragraph>
      <Button
        accessibilityLabel="Confirm Storybook setup"
        className="mt-6 rounded-lg bg-emerald-600 px-4 py-3 text-white hover:bg-emerald-700"
        onPress={() => undefined}
      >
        Confirm
      </Button>
    </View>
  )
}
