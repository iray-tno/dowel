import { Button, Heading, Paragraph, Section, View } from '@hozo/core'

import { accentFor } from '../variants'

export default function Home() {
  return (
    <Section className="p-8 bg-brand">
      <Heading level={1} className="text-3xl font-bold md:hover:text-white">
        Hozo + Next.js
      </Heading>
      <Paragraph className="mt-4">
        Canonical primitives become semantic HTML at build time.
      </Paragraph>
      <View className="mt-6 flex gap-2">
        <Button className="px-4 py-2 rounded-lg">Compiled button</Button>
        {/* The class only exists as another module's return value, so the
            compiler can't read it and the project scan covers it instead. */}
        <View className={accentFor(true)}>Runtime class</View>
      </View>
    </Section>
  )
}
