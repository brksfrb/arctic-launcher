'use strict'

// Replay the recording connection's visual state on the fresh local viewer.
// World-changing client requests remain outside this adapter.
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i
const SWINGS = new Set(['swing_arm', 'wake_up', 'critical_hit', 'magic_critical_hit'])

class ReplaySelf {
  constructor(metadata, player, defaultUuid, runtime, unique) {
    const uuid = metadata.uuid ?? defaultUuid
    if (typeof uuid !== 'string' || !UUID.test(uuid)) throw new Error('Invalid recording-player UUID')
    this.uuids = new Set([uuid.toLowerCase()]); this.player = player
    this.runtime = BigInt(runtime)
    this.unique = typeof unique === 'bigint' ? unique : Number.isSafeInteger(unique) ? BigInt(unique) : null
    this.bound = false; this.started = false; this.pendingSkin = null
    this.equipment = new Map(); this.emotes = null
  }

  viewerUuid() {
    const uuid = this.player.profile?.uuid
    if (typeof uuid !== 'string' || !UUID.test(uuid)) throw new Error('Replay viewer has no valid local UUID')
    return uuid
  }

  ownsUuid(uuid) { return typeof uuid === 'string' && this.uuids.has(uuid.toLowerCase()) }
  ownsRuntime(value) {
    return this.runtime !== null && (typeof value === 'bigint' && value === this.runtime
      || Number.isSafeInteger(value) && value >= 0 && BigInt(value) === this.runtime)
  }

  bind(runtime) {
    if (this.bound || !Number.isSafeInteger(runtime) || runtime < 0 || runtime > 0xFFFFFFFF || BigInt(runtime) !== this.runtime) {
      throw new Error('Replay self state needs one valid world bootstrap')
    }
    this.bound = true
  }

  start() {
    if (!this.bound) throw new Error('Replay self state has no queued world bootstrap')
    this.started = true; this.flushSkin()
    for (const packet of this.equipment.values()) this.player.queue('mob_equipment', packet)
    this.equipment.clear()
    if (this.emotes) { this.player.queue('emote_list', this.emotes); this.emotes = null }
  }

  skin(params) {
    this.pendingSkin = { ...params, uuid: this.viewerUuid() }
  }

  flushSkin() {
    if (!this.started || !this.pendingSkin) return
    const packet = this.pendingSkin
    this.player.queue('player_skin', packet)
    this.pendingSkin = null
  }

  /** null: defer an owned skin packet until the local world exists. Other
   * packets retain their original bytes unless an actual UUID was remapped. */
  server(name, params) {
    if (name === 'player_skin' && this.ownsUuid(params.uuid)) {
      this.skin(params); return null
    }
    if (name !== 'player_list') return params
    const records = params.records
    const modern = Array.isArray(records)
    const list = modern ? records : records?.records
    if (!Array.isArray(list)) throw new Error('Invalid recorded player-list layout')
    let changed = false
    const mapped = list.map(record => {
      const adding = (modern ? record.type : records.type) === 'add'
      const unique = record.entity_unique_id
      const ownEntity = adding && this.unique !== null && (typeof unique === 'bigint' && unique === this.unique
        || Number.isSafeInteger(unique) && BigInt(unique) === this.unique)
      if (!this.ownsUuid(record.uuid) && !ownEntity) return record
      if (typeof record.uuid !== 'string' || !UUID.test(record.uuid)) throw new Error('Invalid recorded self UUID')
      if (!this.ownsUuid(record.uuid) && this.uuids.size >= 16) throw new Error('Recording player changed UUID too often')
      this.uuids.add(record.uuid.toLowerCase()); changed = true
      if (adding && record.skin_data) {
        // The server's player-list skin is an owned protocol asset, including
        // cape images, animated image strips, geometry and persona metadata.
        // Preserve it without reconstructing a skin from authentication data.
        this.skin({ skin: record.skin_data, skin_name: record.skin_data.skin_id ?? '', old_skin_name: '', is_verified: false })
      }
      return { ...record, uuid: this.viewerUuid(), ...(adding ? { xbox_user_id: '', platform_chat_id: '' } : {}) }
    })
    if (!changed) return params
    return { ...params, records: modern ? mapped : { ...records, records: mapped } }
  }

  /** Only the connection owner's visual packets have a server-side projection.
   * They are not dispatched as arbitrary requests or applied to remote actors. */
  client(name, params) {
    if (name === 'player_skin') {
      // This direction belongs to the recording connection. Old recordings
      // may carry its original game UUID rather than the relay's offline UUID;
      // project its skin onto self without adopting that UUID for remote actors.
      if (typeof params.uuid !== 'string' || !UUID.test(params.uuid)) throw new Error('Invalid recorded self skin UUID')
      this.skin(params); this.flushSkin()
      return
    }
    if (name === 'mob_equipment' && this.ownsRuntime(params.runtime_entity_id)) {
      const packet = { ...params, runtime_entity_id: this.runtime }
      if (this.started) this.player.queue(name, packet)
      else this.equipment.set(params.window_id, packet)
    } else if (this.started && name === 'animate' && this.ownsRuntime(params.runtime_entity_id) && SWINGS.has(params.action_id)) {
      if (params.data !== undefined && !Number.isFinite(params.data)) throw new Error('Invalid recorded self animation')
      this.player.queue(name, { ...params, runtime_entity_id: this.runtime })
    } else if (name === 'emote_list' && this.ownsRuntime(params.player_id)) {
      const packet = { ...params, player_id: this.runtime }
      if (this.started) this.player.queue(name, packet)
      else this.emotes = packet
    } else if (this.started && name === 'emote' && this.ownsRuntime(params.entity_id)) {
      if (typeof params.emote_id !== 'string' || !UUID.test(params.emote_id)) throw new Error('Invalid recorded native emote UUID')
      this.player.queue(name, { ...params, entity_id: this.runtime, xuid: '', platform_id: '', flags: 'server_side' })
    }
  }
}

module.exports = { ReplaySelf }
