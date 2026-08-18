import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'
import { hozo } from '@hozo/vite'

// `hozo()` before `react()`: it needs to run against the original
// View/Text/Pressable/Button JSX before @vitejs/plugin-react's own
// transform touches the file.
export default defineConfig({
  plugins: [hozo(), react()],
})
