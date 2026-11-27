import { build } from 'vite'
import assert from 'node:assert/strict'
import {verifyBundle} from './check-bundle.mjs'
import vue from '@vitejs/plugin-vue'
import { mkdir, readFile, writeFile } from 'node:fs/promises'

// The server embeds this one file, so installation needs neither Node nor an asset server.
const result = await build({
  plugins: [vue()],
  define: { 'process.env.NODE_ENV': '"production"' },
  build: {
    lib: {entry: 'src/main.js', formats: ['iife'], name: 'LoomDashboard'},
    target: 'es2020', write: false, minify: 'esbuild',
  },
})
const files = result.flatMap(bundle => bundle.output)
const script = files.find(file => file.type === 'chunk').code
const style = files.find(file => file.fileName.endsWith('.css')).source
const license = await readFile('node_modules/vue/LICENSE', 'utf8')
const index = `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<meta name="color-scheme" content="light dark"><title>Agents · Loom</title>
<style>${style}</style></head><body><div id="app"></div><noscript>Enable JavaScript to view agent activity.</noscript>
<script>/*! Vue license\n${license}\n*/\n${script.replace(/<\/script/gi,'<\\/script')}</script></body></html>\n`
if (process.argv.includes('--check')) {
  assert.equal(await readFile('../dist/index.html','utf8') === index,true,'The rebuilt dashboard differs from ui/dist/index.html.')
  await verifyBundle()
} else {
  await mkdir('../dist', {recursive: true})
  await writeFile('../dist/index.html', index)
  await verifyBundle({write:true})
}
