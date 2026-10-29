import { createHash } from 'node:crypto'
import { copyFile, mkdir, readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

const root = new URL('../', import.meta.url)
const resources = new URL('src-tauri/resources/', root)
const inventory = JSON.parse(
  await readFile(new URL('scripts/tropa-components.json', root), 'utf8'),
)
const pinnedHash = (name) => {
  const component = inventory.components.find(
    (entry) => entry.file === `resources/${name}`,
  )
  if (!component) throw new Error(`Missing pinned component: ${name}`)
  return component.sha256
}
async function copyPinned(source, name, hash) {
  const input = new URL(source, root)
  const bytes = await readFile(input)
  if (createHash('sha256').update(bytes).digest('hex') !== hash) {
    throw new Error(`Unexpected checksum for ${name}: ${fileURLToPath(input)}`)
  }
  await copyFile(input, new URL(name, resources))
}
await mkdir(resources, { recursive: true })
await copyPinned(
  'src-tauri/sidecar/verge-mihomo-x86_64-pc-windows-msvc.exe',
  'verge-mihomo.exe',
  pinnedHash('verge-mihomo.exe'),
)
await mkdir(new URL('licenses/', resources), { recursive: true })
await copyFile(
  new URL('engine/THIRD_PARTY_NOTICES.md', root),
  new URL('licenses/WarpScout.txt', resources),
)
process.stdout.write('Pinned Tropa resources are ready.\n')
