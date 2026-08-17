import { FlatList, StyleSheet, Text, View } from 'react-native'

const rows = ['one', 'two', 'three']

export default function NativeBench() {
  return (
    <View style={styles.root}>
      <Text style={styles.heading}>Bundle benchmark</Text>
      <FlatList
        accessibilityRole="list"
        data={rows}
        keyExtractor={(item) => item}
        renderItem={({ item }) => <Text style={styles.row}>{item}</Text>}
      />
    </View>
  )
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: '#fff',
    padding: 12.8,
  },
  heading: {
    marginBottom: 6.4,
    fontSize: 20,
    lineHeight: 28,
    fontWeight: '700',
  },
  row: {
    padding: 6.4,
  },
})
