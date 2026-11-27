import {createHash} from 'node:crypto'
import {readdir, readFile, writeFile} from 'node:fs/promises'
import {resolve} from 'node:path'
import {fileURLToPath, pathToFileURL} from 'node:url'
import assert from 'node:assert/strict'

const app = fileURLToPath(new URL('.', import.meta.url))
const manifest = resolve(app, '../dist/digest.json')
const hash = bytes => createHash('sha256').update(bytes).digest('hex')

export async function verifyBundle({write=false} = {}) {
  const entries = await readdir(resolve(app,'src'), {recursive:true, withFileTypes:true})
  const sources = ['build.mjs','package.json','package-lock.json', ...entries.filter(entry=>entry.isFile()).map(entry=>
    resolve(entry.parentPath,entry.name).slice(app.length)
  )].sort()
  const inputs = {}
  for (const file of sources) inputs[file] = hash(await readFile(resolve(app,file)))
  const document = {inputs, html:hash(await readFile(resolve(app,'../dist/index.html')))}
  if (write) await writeFile(manifest,JSON.stringify(document,null,2)+'\n')
  else assert.deepEqual(document,JSON.parse(await readFile(manifest,'utf8')),
    'Dashboard input or output changed. Regenerate with npm --prefix ui/app run build.')
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) await verifyBundle()
