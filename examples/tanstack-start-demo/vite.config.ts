import { tanstackStart } from '@tanstack/react-start/plugin/vite'
import viteReact from '@vitejs/plugin-react'
import { nitro } from 'nitro/vite'
import { defineConfig } from 'vite'

import { hozo } from '@hozo/vite'

export default defineConfig({
  plugins: [hozo(), tanstackStart(), viteReact(), nitro()],
})
