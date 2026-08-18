import { renderToString } from 'react-dom/server'
import { Compatibility } from './Compatibility'

export function render() {
  return renderToString(<Compatibility />)
}
