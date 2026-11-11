'use strict'

// Opaque archives already carried by this recording's pack download packets.
// No URL fetch, ZIP extraction, game resource lookup or account access occurs.
const { createHash } = require('node:crypto')
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const VERSION = /^\d{1,10}(?:\.\d{1,10}){2,3}$/
const TRANSFER_PACKETS = new Set(['resource_pack_data_info', 'resource_pack_chunk_data'])
const MAX_ARCHIVE = 512 * 1024 * 1024
const MAX_TOTAL = 1024 * 1024 * 1024
const digest = bytes => createHash('sha256').update(bytes).digest()

function natural(value, maximum, what) {
  // The pinned protocol uses BigInt for lu64. Reject lossy conversions.
  if (typeof value === 'bigint') {
    if (value < 0n || value > BigInt(maximum)) throw new Error(`Invalid recorded resource-pack ${what}`)
    return Number(value)
  }
  if (!Number.isSafeInteger(value) || value < 0 || value > maximum) throw new Error(`Invalid recorded resource-pack ${what}`)
  return value
}

function key(uuid, version) {
  if (typeof uuid !== 'string' || !UUID.test(uuid) || typeof version !== 'string' || !VERSION.test(version)) {
    throw new Error('Invalid recorded resource-pack identity')
  }
  return `${uuid.toLowerCase()}_${version}`
}

function announced(params) {
  const lists = [params.texture_packs ?? [], params.behaviour_packs ?? []]
  if (lists.some(list => !Array.isArray(list)) || lists.reduce((n, list) => n + list.length, 0) > 64) {
    throw new Error('Replay resource-pack offer exceeds 64 packs')
  }
  return lists.flat()
}

class ReplayPacks {
  constructor(replay, parser) { this.replay = replay; this.parser = parser; this.packs = new Map(); this.bytes = 0; this.chunks = 0 }

  static async load(replay, parser, packetId, bootstrapIndex) {
    const archive = new ReplayPacks(replay, parser)
    // These envelope IDs are stable in the supported protocol data. Do not
    // decode inventories before the world's item palette has been installed.
    for (let i = 0; i < bootstrapIndex; i++) {
      if (i % 128 === 0) await new Promise(resolve => setImmediate(resolve))
      const record = replay.records[i]
      if (record.direction !== 0 || ![6, 82, 83].includes(packetId(record))) continue
      const buffer = replay.packet(record)
      const { name, params } = parser.parsePacketBuffer(buffer).data
      if (name === 'resource_packs_info') {
        for (const offer of announced(params)) {
          const id = key(offer.uuid, offer.version), size = natural(offer.size, MAX_ARCHIVE, 'offer size')
          const previous = archive.packs.get(id)
          if (previous && previous.size !== size) throw new Error('Recorded resource-pack identity changes size')
          if (!previous) {
            if (archive.packs.size >= 64 || archive.bytes + size > MAX_TOTAL) throw new Error('Recorded resource packs exceed archive budget')
            archive.packs.set(id, { id, uuid: offer.uuid.toLowerCase(), size, chunks: new Map(), info: null, verified: false })
            archive.bytes += size
          }
        }
      } else if (name === 'resource_pack_data_info') archive.info(params)
      else if (name === 'resource_pack_chunk_data') archive.chunk(params, record, buffer)
      if (i % 32 === 0) await new Promise(resolve => setImmediate(resolve))
    }
    for (const pack of archive.packs.values()) {
      if (!pack.info || pack.chunks.size !== pack.info.chunk_count) {
        throw new Error(`Replay resource-pack archive is missing or incomplete: ${pack.id}`)
      }
      const hash = createHash('sha256')
      for (let i = 0; i < pack.info.chunk_count; i++) {
        const chunk = pack.chunks.get(i)
        if (!chunk) throw new Error('Recorded resource-pack chunk sequence is incomplete')
        hash.update(archive.payload(pack, i).payload)
        if (i % 32 === 0) await new Promise(resolve => setImmediate(resolve))
      }
      if (!hash.digest().equals(pack.info.hash)) throw new Error(`Recorded resource-pack hash mismatch: ${pack.id}`)
      pack.verified = true
    }
    return archive
  }

  resolve(id, allowed = this.packs) {
    if (typeof id !== 'string' || id.length > 100) throw new Error('Invalid resource-pack request identity')
    const direct = this.packs.get(id.toLowerCase())
    if (direct && allowed.has(direct.id)) return direct
    // Some protocol peers request UUID alone, others UUID_version. UUID alone
    // is usable only when the announced version is unambiguous.
    if (UUID.test(id)) {
      const candidates = [...this.packs.values()].filter(p => p.uuid === id.toLowerCase() && allowed.has(p.id))
      if (candidates.length === 1) return candidates[0]
    }
    throw new Error('Resource-pack request does not identify one announced pack')
  }

  info(params) {
    const pack = this.resolve(params.pack_id)
    const size = natural(params.size, MAX_ARCHIVE, 'data size')
    const stride = natural(params.max_chunk_size, 4 * 1024 * 1024, 'chunk size')
    const count = natural(params.chunk_count, 65536, 'chunk count')
    if (!size || size !== pack.size || !stride || count !== Math.ceil(size / stride)
      || !Buffer.isBuffer(params.hash) || params.hash.length !== 32) throw new Error('Invalid recorded resource-pack download descriptor')
    if (pack.info) {
      if (pack.info.max_chunk_size !== stride || pack.info.chunk_count !== count || !pack.info.hash.equals(params.hash)
        || pack.info.is_premium !== params.is_premium || pack.info.pack_type !== params.pack_type) {
        throw new Error('Conflicting recorded resource-pack download descriptors')
      }
    } else pack.info = params
  }

  chunk(params, record, buffer) {
    const pack = this.resolve(params.pack_id), info = pack.info
    if (!info) throw new Error('Recorded resource-pack chunk precedes its descriptor')
    const index = natural(params.chunk_index, info.chunk_count - 1, 'chunk index')
    const expected = Math.min(info.max_chunk_size, pack.size - index * info.max_chunk_size)
    natural(params.progress, pack.size, 'download progress')
    if (!Buffer.isBuffer(params.payload) || params.payload.length !== expected) throw new Error('Invalid recorded resource-pack payload size')
    const hash = digest(params.payload), previous = pack.chunks.get(index)
    if (previous && !previous.hash.equals(hash)) throw new Error('Conflicting recorded resource-pack chunks')
    if (!previous) {
      if (++this.chunks > 65536) throw new Error('Recorded resource packs exceed chunk-index budget')
      pack.chunks.set(index, { record, hash, packetHash: digest(buffer) })
    }
  }

  payload(pack, index) {
    const chunk = pack.chunks.get(index)
    if (!chunk) throw new Error('Resource-pack archive has a missing chunk')
    const buffer = this.replay.packet(chunk.record)
    if (!digest(buffer).equals(chunk.packetHash)) throw new Error('Recorded resource-pack packet changed after indexing')
    const { name, params } = this.parser.parsePacketBuffer(buffer).data
    // Recheck bytes on demand: the open recording could have been modified
    // after indexing. The recorder's filename is never an integrity receipt.
    if (name !== 'resource_pack_chunk_data' || params.chunk_index !== index
      || this.resolve(params.pack_id).id !== pack.id || !Buffer.isBuffer(params.payload)
      || !digest(params.payload).equals(chunk.hash)) throw new Error('Recorded resource-pack bytes changed after indexing')
    return params
  }

  session(player) { return new ReplayPackSession(this, player) }
  status() {
    const embedded = [...this.packs.values()].filter(p => p.verified)
    return { offered: this.packs.size, embedded: embedded.length, offeredBytes: this.bytes,
      embeddedBytes: embedded.reduce((bytes, pack) => bytes + pack.size, 0) }
  }
}

class ReplayPackSession {
  constructor(archive, player) {
    this.archive = archive; this.player = player; this.allowed = new Set(); this.requested = new Set()
    this.phase = 'idle'; this.pending = []; this.retries = new Map()
  }

  offer(params) {
    if (this.phase !== 'idle') throw new Error('Unexpected replay resource-pack offer')
    for (const offer of announced(params)) {
      const id = key(offer.uuid, offer.version), pack = this.archive.packs.get(id)
      if (!pack?.verified || pack.size !== natural(offer.size, MAX_ARCHIVE, 'offer size')) {
        throw new Error(`Replay resource-pack archive is missing or incomplete: ${id}`)
      }
      if (this.allowed.has(id)) throw new Error('Duplicate replay resource-pack offer')
      this.allowed.add(id)
    }
    this.phase = 'offered'
    // Keep content keys/identities and stack ordering. Fetch only from this
    // loopback replay, even when the original offer advertised a CDN URL.
    const out = { ...params }
    for (const field of ['texture_packs', 'behaviour_packs']) if (Array.isArray(params[field])) {
      out[field] = params[field].map(pack => ({ ...pack, ...(Object.hasOwn(pack, 'cdn_url') ? { cdn_url: '' } : {}) }))
    }
    return out
  }

  stack(params) {
    if (this.phase !== 'have_all') throw new Error('Replay resource-pack stack precedes client acceptance')
    const list = [...(params.resource_packs ?? []), ...(params.behavior_packs ?? [])]
    if (list.length > 64) throw new Error('Replay resource-pack stack exceeds 64 packs')
    for (const item of list) {
      const id = key(item.uuid, item.version)
      if (!this.allowed.has(id)) throw new Error('Replay stack references an unannounced resource pack')
    }
    this.phase = 'stack'
  }

  response(packet) {
    if (packet.response_status === 'refused') throw new Error('Replay resource packs were refused')
    if (packet.response_status === 'send_packs') {
      if (!['offered', 'downloading'].includes(this.phase) || !Array.isArray(packet.resourcepackids)
        || !packet.resourcepackids.length || packet.resourcepackids.length > this.allowed.size) throw new Error('Invalid replay pack download request')
      const requested = new Set()
      const replies = packet.resourcepackids.map(id => {
        const pack = this.archive.resolve(id, this.allowed)
        if (requested.has(pack.id)) throw new Error('Duplicate replay pack download request')
        requested.add(pack.id)
        return { pack, id }
      })
      for (const { pack, id } of replies) {
        if (this.requested.has(pack.id)) throw new Error('Replay pack download was already requested')
        this.player.queue('resource_pack_data_info', { ...pack.info, pack_id: id })
        this.requested.add(pack.id)
      }
      this.phase = 'downloading'; return false
    }
    if (['offered', 'downloading'].includes(this.phase)
      && (packet.response_status === 'have_all_packs' || packet.response_status === 'completed' && this.allowed.size === 0)) {
      this.phase = 'have_all'; this.pending.length = 0; return true
    }
    if (this.phase === 'stack' && packet.response_status === 'completed') { this.phase = 'ready'; return true }
    throw new Error('Unexpected replay resource-pack response')
  }

  request(packet) {
    if (this.phase !== 'downloading' || this.pending.length >= 256) throw new Error('Replay pack chunk request is out of phase or exceeds queue budget')
    const pack = this.archive.resolve(packet.pack_id, this.requested)
    const index = natural(packet.chunk_index, pack.info.chunk_count - 1, 'requested chunk index')
    const id = `${pack.id}:${index}`, attempts = (this.retries.get(id) ?? 0) + 1
    if (attempts > 8) throw new Error('Replay resource-pack chunk retry limit exceeded')
    this.retries.set(id, attempts); this.pending.push({ pack, index, id: packet.pack_id })
  }

  drain() {
    let bytes = 0, count = 0
    while (this.pending.length && bytes < 256 * 1024 && count < 8) {
      const { pack, index, id } = this.pending.shift(), params = this.archive.payload(pack, index)
      this.player.queue('resource_pack_chunk_data', { ...params, pack_id: id })
      bytes += params.payload.length; count++
    }
    return count
  }
}

module.exports = { ReplayPacks, TRANSFER_PACKETS }
