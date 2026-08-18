import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Login } from './Login'
import { Panel } from './Panel'
import { Compatibility } from './Compatibility'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <Login />
    <Panel show items={["a", "b"]} />
    <Compatibility />
  </StrictMode>,
)
