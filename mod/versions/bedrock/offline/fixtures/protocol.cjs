'use strict'
// Schema-generated packet fixtures shared by headless network/CLI checks.
module.exports = function fixture(version, name) {
  const types = require('minecraft-data')(`bedrock_${version}`).protocol.types
  const zero = type => {
    if (typeof type === 'string') {
      if (['nbt', 'nbtLoop', 'lnbt'].includes(type)) return { type: 'compound', name: '', value: {} }
      if (type === 'uuid') return '00000000-0000-0000-0000-000000000000'
      if (type === 'string') return ''
      if (type === 'bool') return false
      if (['lu64', 'li64', 'varint64', 'zigzag64'].includes(type)) return 0n
      return types[type] && types[type] !== 'native' ? zero(types[type]) : 0
    }
    const [kind, args] = type
    if (kind === 'container') return Object.fromEntries(args.map(f => [f.name, zero(f.type)]))
    if (kind === 'array') return []
    if (kind === 'mapper') return Object.values(args.mappings)[0]
    if (kind === 'bitfield' || kind === 'bitflags') return Object.fromEntries((args?.flags || args || []).map?.(f => [f.name || f, 0]) || [])
    if (kind === 'buffer') return Buffer.alloc(0)
    if (kind === 'switch' || kind === 'option') return undefined
    return 0
  }
  return zero(types[`packet_${name}`])
}
