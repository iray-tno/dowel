import { Image, ScrollView, Text, View } from '@dowel/core'

const imageSource = {
  uri: 'data:image/svg+xml,%3Csvg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/%3E',
}

/** Production/SSR fixture for the opt-in Web compatibility adapters. */
export function Compatibility() {
  return (
    <View
      className="hidden"
      testID="compatibility-root"
      nativeID="compatibility"
      pointerEvents="box-none"
      accessibilityState={{ busy: false }}
      accessibilityLiveRegion="polite"
      onLayout={() => {}}
    >
      <Image src={imageSource} defaultSource={imageSource} alt="Compatibility fixture" />
      <ScrollView onScroll={() => {}} scrollEventThrottle={16}>
        <Text>Universal Web adapter</Text>
      </ScrollView>
    </View>
  )
}
