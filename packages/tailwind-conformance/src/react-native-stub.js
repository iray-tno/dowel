// Stands in for `react-native` when the Native output is rendered.
//
// React Native ships Flow-typed JavaScript that Node cannot parse, so the
// real package is not importable here. See `./native-render.ts` for what
// that means for what these tests establish -- in short, the tree Dowel
// builds is checked, React Native's runtime is not.
//
// The components are strings, which `react-test-renderer` reports as host
// elements: `<View style={...}>` comes back as
// `{ type: 'View', props: { style } }`, so an assertion can be about what
// Dowel put there rather than about what React Native did with it.

export const View = 'View'
export const Text = 'Text'
export const Pressable = 'Pressable'
export const TextInput = 'TextInput'
export const Modal = 'Modal'

export const StyleSheet = {
  // Identity, deliberately. The real `create` returns opaque registry
  // values; the point here is to read back the style Dowel wrote. Whether
  // React Native would accept it is the type check's question, asked
  // against its declarations rather than its runtime.
  create: (styles) => styles,
  flatten: (style) => Object.assign({}, ...(Array.isArray(style) ? style.filter(Boolean) : [style || {}])),
}

export const Dimensions = {
  get: () => ({ width: 390, height: 844, scale: 3, fontScale: 1 }),
  addEventListener: () => ({ remove: () => {} }),
}

export const Appearance = {
  getColorScheme: () => 'light',
  addChangeListener: () => ({ remove: () => {} }),
}

export const Easing = {
  linear: (value) => value,
  ease: (value) => value,
  in: (easing) => easing,
  out: (easing) => easing,
  inOut: (easing) => easing,
}

export const Animated = {
  Value: class {
    constructor(value) {
      this.value = value
    }
    interpolate(config) {
      return { __animatedInterpolation: config }
    }
  },
  createAnimatedComponent: (component) => component,
  timing: () => ({ start: () => {}, stop: () => {} }),
  loop: (animation) => animation,
  parallel: () => ({ start: () => {}, stop: () => {} }),
}
