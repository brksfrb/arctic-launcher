'use strict'

// Decrypted game packets, not login envelopes/session keys or RakNet datagrams.
// This format is Bedrock-only. It is deliberately not mislabeled Java .mcpr.
const fs = require('node:fs')
const { isAbsolute, extname } = require('node:path')
const { once } = require('node:events')
const { randomUUID, createHash } = require('node:crypto')
const camera = require('./camera.cjs')
const { ReplayPacks, TRANSFER_PACKETS } = require('./replay-packs.cjs')
const { ReplaySelf } = require('./replay-self.cjs')
const MAGIC = Buffer.from('ARCTICBR\x01')
const MAX_PACKET = 16 * 1024 * 1024
const MAX_FILE = 2 * 1024 * 1024 * 1024
const MAX_TIME = 24 * 60 * 60 * 1000
// Java ReplayViewer offers 0.25/0.5/1/2/4/8/16x and caps real frame time
// before scaling it. Keep legacy continuous Bedrock speeds down to 0.1x.
const MIN_SPEED = 0.1, MAX_SPEED = 16, MAX_FRAME_MS = 250
const AUTH = new Set(['login', 'server_to_client_handshake', 'client_to_server_handshake',
  'network_settings', 'request_network_settings'])
// A recording must never transfer its viewer away from this local server or
// reuse timing probes belonging to the original network connection.
const CONNECTION_CONTROL = new Set(['transfer', 'network_stack_latency', 'tick_sync'])

function sameFile(a, b) { return a.dev === b.dev && a.ino === b.ino && a.size === b.size && a.mtimeNs === b.mtimeNs && a.ctimeNs === b.ctimeNs }
function hashDescriptor(fd, expected) {
  const before = fs.fstatSync(fd, { bigint: true })
  if (!sameFile(before, expected) || before.size > BigInt(MAX_FILE)) throw new Error('Replay source changed before hashing')
  const hash = createHash('sha256'), buffer = Buffer.alloc(65536), size = Number(before.size)
  for (let at = 0; at < size;) {
    const read = fs.readSync(fd, buffer, 0, Math.min(buffer.length, size - at), at)
    if (read < 1) throw new Error('Replay source was truncated while hashing')
    hash.update(buffer.subarray(0, read)); at += read
  }
  if (!sameFile(before, fs.fstatSync(fd, { bigint: true }))) throw new Error('Replay source changed while hashing')
  return hash.digest('hex')
}

class PacketRecording {
  constructor(file, metadata, onError = () => {}) {
    if (typeof file !== 'string' || !isAbsolute(file)) throw new Error('Replay output must be an absolute path')
    this.metadata = { format: 1, edition: 'bedrock', ...metadata }
    if (metadata.recordingId != null) {
      if (typeof metadata.recordingId !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(metadata.recordingId)) throw new Error('Invalid replay recording identity')
      this.metadata.recordingId = metadata.recordingId.toLowerCase()
    }
    const header = Buffer.from(JSON.stringify(this.metadata))
    if (header.length > 4096) throw new Error('Replay metadata exceeds 4 KiB')
    // Exclusive creation: neither recordings nor arbitrary existing files get overwritten.
    const fd = fs.openSync(file, 'wx')
    this.file = file; this.createdFile = fs.fstatSync(fd, { bigint: true })
    this.stream = fs.createWriteStream(null, { fd, autoClose: true })
    this.stream.on('error', error => { this.error = error; onError(error) })
    this.started = performance.now()
    this.bytes = 0
    this.count = 0
    this.ended = false
    this.onError = onError
    const size = Buffer.alloc(4); size.writeUInt32BE(header.length)
    this.write(Buffer.concat([MAGIC, size, header]))
  }
  write(buffer) {
    if (this.error || this.stream.destroyed) return false
    if (this.bytes + buffer.length > MAX_FILE || this.stream.writableLength + buffer.length > 32 * 1024 * 1024) {
      this.stream.destroy(new Error('Replay recording reached its file/queue limit'))
      return false
    }
    this.bytes += buffer.length
    return this.stream.write(buffer)
  }
  packet(direction, name, data) {
    return this.packetAt(Math.floor(performance.now() - this.started), direction, name, data)
  }
  position() { return Math.min(MAX_TIME, Math.floor(performance.now() - this.started)) }
  digest() {
    if (!this.ended || !this.stream.closed || this.error) throw new Error('Replay must finalize before hashing')
    const fd = fs.openSync(this.file, 'r')
    try {
      const stat = fs.fstatSync(fd, { bigint: true })
      if (stat.dev !== this.createdFile.dev || stat.ino !== this.createdFile.ino || stat.size !== BigInt(this.bytes)) throw new Error('Finalized replay path no longer identifies the written file')
      return hashDescriptor(fd, stat)
    } finally { fs.closeSync(fd) }
  }
  packetAt(time, direction, name, data) {
    if (this.ended || this.error || AUTH.has(name) || !Buffer.isBuffer(data)) return
    if (data.length < 1 || data.length > MAX_PACKET || !Number.isSafeInteger(time) || time < (this.lastTime ?? 0)
      || time > MAX_TIME || ![0, 1].includes(direction) || this.count >= 1_000_000) {
      this.stream.destroy(new Error('Replay recording exceeded packet/time limits')); return
    }
    this.lastTime = time
    const header = Buffer.alloc(9)
    header.writeUInt32BE(time); header.writeUInt32BE(data.length, 4); header[8] = direction
    const accepted = this.write(Buffer.concat([header, data])); this.count++
    return accepted
  }
  finish(duration = Math.max(this.lastTime ?? 0, Math.min(MAX_TIME, Math.floor(performance.now() - this.started)))) {
    if (this.finishing) return this.finishing
    if (!Number.isSafeInteger(duration) || duration < (this.lastTime ?? 0) || duration > MAX_TIME) return Promise.reject(new Error('Invalid replay duration'))
    this.ended = true
    this.duration = duration
    this.finishing = (async () => {
      if (this.error) throw this.error
      if (this.stream.closed) throw new Error('Replay output closed before it could be finalized')
      const closed = once(this.stream, 'close')
      // Zero-length packet seals the recording. An interrupted file is refused.
      const footer = Buffer.alloc(9)
      footer.writeUInt32BE(duration)
      this.write(footer)
      this.stream.end()
      await closed
      if (this.error) throw this.error
    })()
    return this.finishing
  }
}

class PacketReplay {
  constructor(file) {
    this.fd = fs.openSync(file, 'r')
    this.records = []
    try {
      const initial = fs.fstatSync(this.fd, { bigint: true }), size = Number(initial.size)
      if (size < MAGIC.length + 4 || size > MAX_FILE) throw new Error('Invalid replay size')
      if (!this.read(0, MAGIC.length).equals(MAGIC)) throw new Error('Not an Arctic Bedrock replay')
      const len = this.read(MAGIC.length, 4).readUInt32BE()
      if (len > 4096 || MAGIC.length + 4 + len > size) throw new Error('Invalid replay metadata size')
      this.metadata = JSON.parse(this.read(MAGIC.length + 4, len).toString('utf8'))
      const m = this.metadata
      if (m.format !== 1 || m.edition !== 'bedrock' || typeof m.version !== 'string'
        || !/^\d+\.\d+\.\d+(?:\.\d+)?$/.test(m.version)
        || typeof m.username !== 'string' || !/^[A-Za-z0-9_]{3,16}$/.test(m.username)
        || m.uuid != null && (typeof m.uuid !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(m.uuid))
        || m.recordingId != null && (typeof m.recordingId !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/.test(m.recordingId))) throw new Error('Invalid Bedrock replay metadata')
      let at = MAGIC.length + 4 + len, previous = 0, sealed = false
      while (at < size) {
        if (at + 9 > size) throw new Error('Replay is truncated')
        const header = this.read(at, 9), time = header.readUInt32BE(), length = header.readUInt32BE(4), direction = header[8]
        at += 9
        if (time < previous || time > MAX_TIME || length > MAX_PACKET || direction > 1 || at + length > size) throw new Error('Invalid replay record')
        previous = time
        if (length === 0) { sealed = at === size && direction === 0; break }
        if (this.records.length >= 1_000_000) throw new Error('Replay has too many packets')
        this.records.push({ time, length, direction, offset: at })
        at += length
      }
      if (!sealed) throw new Error('Replay was not finalized; keeping the original file for recovery')
      this.duration = previous
      if (!sameFile(initial, fs.fstatSync(this.fd, { bigint: true }))) throw new Error('Replay changed while its packet index was read')
      this.sourceStat = initial
    } catch (error) { this.close(); throw error }
  }
  read(offset, length) {
    if (this.sourceStat && !sameFile(this.sourceStat, fs.fstatSync(this.fd, { bigint: true }))) throw new Error('Replay source changed after indexing')
    const buffer = Buffer.allocUnsafe(length)
    if (fs.readSync(this.fd, buffer, 0, length, offset) !== length) throw new Error('Replay is truncated')
    return buffer
  }
  packet(record) { return this.read(record.offset, record.length) }
  digest() {
    if (!sameFile(this.sourceStat, fs.fstatSync(this.fd, { bigint: true }))) throw new Error('Replay source changed after indexing')
    return this.sourceHash ??= hashDescriptor(this.fd, this.sourceStat)
  }
  close() { if (this.fd != null) { fs.closeSync(this.fd); this.fd = null } }
}

function validatePlayback(input) {
  if (!input || typeof input !== 'object' || Object.keys(input).some(k => !['file', 'listenPort', 'speed', 'startMs', 'host', 'transport', 'waitForClientInit'].includes(k))) throw new Error('Invalid replay configuration')
  const transport = input.transport ?? 'raknet'
  if (!['raknet', 'nethernet'].includes(transport)) throw new Error('Invalid replay transport')
  const { file, listenPort = transport === 'nethernet' ? 7551 : 19143, speed = 1, startMs = 0,
    host = transport === 'nethernet' ? '127.0.0.2' : '127.0.0.1', waitForClientInit = transport === 'nethernet' } = input
  if (typeof waitForClientInit !== 'boolean') throw new Error('Invalid replay initialization policy')
  if (!['127.0.0.1', '127.0.0.2'].includes(host)) throw new Error('Replay host must be a private IPv4 loopback interface')
  if (typeof file !== 'string' || !isAbsolute(file)) throw new Error('Replay input must be an absolute path')
  if (!Number.isInteger(listenPort) || listenPort < 1 || listenPort > 65535) throw new Error('Invalid replay port')
  if (!Number.isFinite(speed) || speed < MIN_SPEED || speed > MAX_SPEED) throw new Error('Replay speed must be between 0.1 and 16')
  if (!Number.isSafeInteger(startMs) || startMs < 0 || startMs > MAX_TIME) throw new Error('Invalid replay start time')
  return { file, listenPort, speed, startMs, host, transport, waitForClientInit }
}

// Export a self-contained packet replay with a baked camera track. This
// preserves terrain/entity packets and timing; it is not a rendered video file.
async function exportCameraReplay(file, output, path, options = {}) {
  if (!options || typeof options !== 'object' || Array.isArray(options)
    || Object.keys(options).some(key => key !== 'fps')) throw new Error('Invalid replay export options')
  const fps = options.fps ?? 25
  if (!Number.isInteger(fps) || fps < 1 || fps > 120) throw new Error('Camera export FPS must be an integer between 1 and 120')
  if (typeof output !== 'string' || !isAbsolute(output) || extname(output).toLowerCase() !== '.abr') {
    throw new Error('Edited replay export needs an absolute .abr filename; MP4 uses native video capture')
  }
  const replay = new PacketReplay(file)
  let recording
  try {
    require('./connector.cjs').loadProtocol()
    if (replay.metadata.version !== '1.26.51') throw new Error('Camera editing currently requires protocol data 1.26.51 (2193)')
    path = camera.cameraPath(path, replay.duration)
    if (replay.records.length + Math.ceil(replay.duration * fps / 1000) + 4 > 1_000_000) throw new Error('Edited replay would exceed its packet limit')
    const { createSerializer, createDeserializer } = require('bedrock-protocol/src/transforms/serializer')
    const serializer = createSerializer(replay.metadata.version), parser = createDeserializer(replay.metadata.version)
    // Preserve recording metadata; adding the export's wall-clock time made
    // identical edits produce different files. Canonicalize object key order so
    // equivalent inputs also have an identical header byte representation.
    const metadata = Object.fromEntries(Object.entries({ ...replay.metadata,
      cameraExport: { format: 1, fps } }).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0))
    recording = new PacketRecording(output, metadata)
    const write = async (time, direction, name, data) => {
      if (!recording.packetAt(time, direction, name, data)) {
        if (recording.error) throw recording.error
        await once(recording.stream, 'drain')
      }
    }
    const generated = (time, name, params) => write(time, 0, name, serializer.createPacketBuffer({ name, params }))
    let started = false, next = 0, sampleIndex = 0, origin = 0, preset = camera.presets(), sentPresets = false
    const samples = async until => {
      while (started && next < until && next <= replay.duration) {
        // The record keeps a whole-millisecond timestamp, but its camera pose
        // belongs to the exact output frame, including the fractional ms.
        const frameTime = camera.frameTime(origin, sampleIndex, fps)
        await generated(frameTime.timeMs, 'camera_instruction', camera.instruction(camera.sampleForViewAt(path, frameTime), preset.id))
        // Derive each timestamp from its integer frame index. Repeatedly adding
        // 1000/fps accumulates rounding drift at rates such as 30/60/120 Hz.
        next = origin + Math.floor(++sampleIndex * 1000 / fps)
      }
    }
    for (let index = 0; index < replay.records.length; index++) {
      const record = replay.records[index], buffer = replay.packet(record)
      await samples(record.time)
      const { name, params } = parser.parsePacketBuffer(buffer).data
      // Item extra data depends on this world's shield runtime ID. Unlike a
      // live Connection, this standalone decoder has no palette listener.
      // Apply the registry before decoding later recipes/inventory packets.
      if (record.direction === 0 && (name === 'item_registry' || name === 'start_game')) {
        const shield = params.itemstates?.find(item => item.name === 'minecraft:shield')
        if (shield && Number.isInteger(shield.runtime_id)) {
          parser.proto.setVariable('ShieldItemID', shield.runtime_id)
        }
      }
      if (AUTH.has(name)) continue
      if (record.direction === 0 && camera.CAMERA_PACKETS.has(name)) continue
      if (record.direction === 0 && name === 'camera_presets') {
        preset = camera.presets(params.presets); sentPresets = true
        await generated(record.time, name, { presets: preset.presets })
      } else {
        await write(record.time, record.direction, name, buffer)
      }
      if (record.direction === 0 && name === 'start_game') {
        if (started) throw new Error('Edited replay contains multiple world bootstraps')
        started = true; origin = record.time; next = origin
        if (!sentPresets) await generated(record.time, 'camera_presets', { presets: preset.presets })
      }
      // Keep control/stop input responsive during large exports.
      if (index % 128 === 0) await new Promise(resolve => setImmediate(resolve))
    }
    if (!started) throw new Error('Replay contains no world bootstrap')
    await samples(replay.duration)
    await generated(replay.duration, 'camera_instruction', camera.instruction(camera.sampleForView(path, replay.duration), preset.id))
    await recording.finish(replay.duration)
  } catch (error) {
    if (recording && !recording.stream.closed) {
      const closed = once(recording.stream, 'close').catch(() => {})
      recording.stream.destroy(); await closed
    }
    throw error
  } finally { replay.close() }
}

async function startPlayback(input, emit = () => {}) {
  const config = validatePlayback(input)
  const replay = new PacketReplay(config.file)
  const { loadProtocol, bindOfflinePeer, offlineUuid } = require('./connector.cjs')
  let server, nethernet
  try {
    const protocol = loadProtocol()
    const { Versions } = require('bedrock-protocol/src/options')
    if (!Versions[replay.metadata.version]) throw new Error('Replay protocol is not supported')
    if (config.startMs > replay.duration) throw new Error('Replay start time exceeds its duration')
    const packetId = record => {
      const header=replay.read(record.offset,Math.min(5,record.length))
      let value=0
      for(let i=0;i<header.length;i++) {
        value|=(header[i]&127)<<(7*i)
        if(!(header[i]&128))return value&1023
      }
      throw new Error('Invalid recorded packet header')
    }
    // These packet IDs are defined by the pinned Bedrock protocol data. Read
    // only these envelopes here: inventory decoding needs the world palette.
    const bootstrapIndex = replay.records.findIndex(record => record.direction === 0 && packetId(record) === 11)
    if (bootstrapIndex < 0) throw new Error('Replay contains no world bootstrap; its connection did not reach a world')
    server = new protocol.Server({ host: config.host, port: config.listenPort, version: replay.metadata.version,
      transport: config.transport,
      // One active viewer plus a retiring transfer peer. Admission below still
      // rejects a second active world; the extra slot permits a fresh handshake.
      offline: true, raknetBackend: 'jsp-raknet', useRaknetWorkers: false, maxPlayers: 2,
      motd: { motd: 'Arctic Bedrock Replay', levelName: 'Offline playback' } })
    if (config.transport === 'nethernet') {
      nethernet = require('./replay-nethernet.cjs').configureReplayNethernet(server, config)
    }
    const resourcePacks = await ReplayPacks.load(replay, server.deserializer, packetId, bootstrapIndex)
    const bootstrap = replay.records[bootstrapIndex]
    const original = server.deserializer.parsePacketBuffer(replay.packet(bootstrap)).data.params
    const originalRuntime = Number(original.runtime_entity_id)
    if (!Number.isSafeInteger(originalRuntime) || originalRuntime < 0 || originalRuntime > 0xFFFFFFFF) throw new Error('Recorded player runtime id exceeds this replay protocol adapter')
    let initializedIndex = -1
    if (config.waitForClientInit) {
      initializedIndex = replay.records.findIndex((record, index) => index > bootstrapIndex && record.direction === 1 && packetId(record) === 113)
      if (initializedIndex < 0) throw new Error('Native replay initialization requires a recorded local-player initialization checkpoint')
      const checkpoint = server.deserializer.parsePacketBuffer(replay.packet(replay.records[initializedIndex])).data.params
      if (String(original.runtime_entity_id) !== String(checkpoint.runtime_entity_id)) throw new Error('Replay initialization checkpoint belongs to a different player')
    }
    const bootstrapTime = config.waitForClientInit ? replay.records[initializedIndex].time : bootstrap.time
    const timers = new Set(), viewers = new Set(), exports = new Set(), markers = new Set(), retiring = new Map()
    let speed = config.speed, paused = false, path = null, fixedCamera = null, cameraRevision = 0, cameraPathSha256 = null, closed = false
    let startMs = Math.max(config.startMs, bootstrapTime)
    let generation = 1, lastStepSequence = 0, pendingStep = null
    let pendingReset = null, lastReset = null
    const resetState = () => pendingReset ? { generation: pendingReset.generation, timeMs: pendingReset.timeMs,
      stage: pendingReset.stage, automaticReconnect: true, oldViewerId: pendingReset.oldViewerId,
      newViewerId: pendingReset.newViewerId ?? null } : lastReset
    const status = () => ({ event: 'replay_state', positionMs: viewers.values().next().value?.position() ?? startMs,
      recordingId: replay.metadata.recordingId ?? null, recordingFile: config.file,
      durationMs: replay.duration, paused, speed, camera: path ? 'path' : fixedCamera ? 'fixed' : 'recorded', xboxAuth: false,
      cameraRevision, cameraPathSha256,
      nativeWorldVerified: false,
      automaticReconnectAvailable: config.transport === 'raknet' && replay.metadata.version === '1.26.51',
      worldReset: resetState(),
      resourcePacks: resourcePacks.status(),
      generation, connectionRequired: viewers.size === 0, packetReady: viewers.size > 0 && [...viewers].every(v => v.ready()),
      bootstrapTimeMs: bootstrapTime, waitForClientInit: config.waitForClientInit,
      clientInitialized: viewers.size > 0 && [...viewers].every(v => v.initialized()),
      viewers: viewers.size, initializedViewers: [...viewers].filter(v => v.initialized()).length,
      viewerIdentities: [...viewers].filter(v => v.identity()).map(v => v.identity()),
      pendingStep: pendingStep ? { generation, sequence: pendingStep.sequence, timeMs: pendingStep.timeMs } : null })
    const failReset = message => {
      if (!pendingReset) return
      const reset = pendingReset; clearTimeout(reset.timeout)
      lastReset = { ...resetState(), stage: 'failed', message }; pendingReset = null
      for (const viewer of [...viewers]) if (viewer.identity()?.viewerId === reset.newViewerId) viewer.stop(message)
      emit({ event: 'replay_reset_failed', generation: reset.generation, worldReset: lastReset,
        message, nativeWorldVerified: false, xboxAuth: false })
      emit(status())
    }
    const advanceReset = viewer => {
      if (!pendingReset && lastReset?.generation === generation
          && ['awaiting_connection', 'failed'].includes(lastReset.stage)
          && viewer.initialized() && viewer.ready() && viewer.position() >= lastReset.timeMs) {
        lastReset = { ...lastReset, stage: 'ready', automaticReconnect: false,
          message: undefined, newViewerId: viewer.identity()?.viewerId }
        emit({ event: 'replay_reset_ready', generation, worldReset: lastReset,
          positionMs: viewer.position(), clientInitialized: true, packetsQueued: true,
          nativeWorldVerified: false, xboxAuth: false })
        emit(status())
      }
      if (!pendingReset || pendingReset.generation !== generation || viewer.identity()?.viewerId !== pendingReset.newViewerId) return
      if (!viewer.initialized()) return
      pendingReset.stage = 'catching_up'
      if (!viewer.ready() || viewer.position() !== pendingReset.timeMs) return
      clearTimeout(pendingReset.timeout)
      lastReset = { ...resetState(), stage: 'ready' }; pendingReset = null
      // This proves a fresh connection initialized and packets were queued.
      // Native terrain, simulation, rendering and repeatability are separate.
      emit({ event: 'replay_reset_ready', generation, worldReset: lastReset,
        positionMs: viewer.position(), clientInitialized: true, packetsQueued: true,
        nativeWorldVerified: false, xboxAuth: false })
      emit(status())
    }
    const cancelStep = message => {
      if (!pendingStep) return
      const step = pendingStep; pendingStep = null; clearTimeout(step.timeout)
      step.reject(new Error(message))
    }
    const completeStep = () => {
      if (pendingStep?.marker && (viewers.size !== 1 || !viewers.has(pendingStep.viewer) || !pendingStep.viewer.initialized())) {
        cancelStep('Native replay marker lost its original initialized viewer'); return
      }
      if (!pendingStep || !viewers.size || ![...viewers].every(v => v.ready() && v.position() === pendingStep.timeMs
        && (!pendingStep.cameraTime || !(path || fixedCamera) || v.cameraPosition() === pendingStep.cameraAt))) return
      const step = pendingStep; pendingStep = null; clearTimeout(step.timeout)
      const event = { event: 'replay_step', generation, sequence: step.sequence, positionMs: step.timeMs,
        packetsQueued: true, nativeWorldVerified: false, xboxAuth: false }
      if (step.cameraTime) {
        event.cameraTime = { timeMs: step.timeMs, ...step.cameraTime }
        event.cameraQueued = !!(path || fixedCamera)
        if (event.cameraQueued) event.cameraPose = viewers.values().next().value.cameraPose()
      }
      if (step.marker) {
        try {
          const viewer = step.viewer
          viewer.marker(step.marker)
          event.markerTimestamp = step.marker; event.markerQueued = true
          event.viewerId = viewer.identity()?.viewerId
        } catch (error) { step.reject(error); throw error }
      }
      emit(event); step.resolve(event)
    }
    const applyControl = async command => {
      if (closed) throw new Error('Replay is stopped')
      const fields = { pause: ['paused'], speed: ['speed'], seek: ['timeMs'], camera: ['pose'],
        camera_path: ['path'], camera_clear: [], state: [], recording_source: [], export: ['file', 'path', 'fps'],
        reset: ['timeMs', 'reconnect'], cancel_reset: ['generation', 'oldViewerId'],
        cancel_step: ['generation', 'sequence'], step: ['generation', 'sequence', 'timeMs', 'marker', 'cameraTime'] }
      if (!command || !Object.hasOwn(fields, command.op) || Object.keys(command).some(key => key !== 'op' && !fields[command.op].includes(key))) throw new Error('Invalid replay control')
      if (pendingStep && !['state', 'reset', 'cancel_step'].includes(command.op)) throw new Error('Wait for the pending replay step before changing playback')
      if (pendingReset && !['state', 'reset', 'cancel_reset', 'recording_source'].includes(command.op)) throw new Error('Wait for fresh-world reconnection and bootstrap before changing replay playback')
      switch (command.op) {
        case 'state': {
          const state = status(); emit(state); return state
        }
        case 'recording_source': return { recordingId: replay.metadata.recordingId ?? null, recordingFile: config.file, sourceSha256: replay.digest() }
        case 'cancel_reset': {
          const reset = pendingReset ?? lastReset
          if (!reset || command.generation !== generation || command.generation !== reset.generation
              || typeof command.oldViewerId !== 'string' || command.oldViewerId !== reset.oldViewerId) throw new Error('Reset cancellation belongs to another world transition')
          if (pendingReset) failReset('Replay reconnect canceled by its owner')
          break
        }
        case 'cancel_step':
          if (command.generation !== generation || !Number.isSafeInteger(command.sequence)
            || command.sequence < 0 || command.sequence !== lastStepSequence
            || pendingStep && command.sequence !== pendingStep.sequence) throw new Error('Cancel belongs to a different replay step')
          cancelStep('Replay step canceled by its owner'); break
        case 'pause':
          if (typeof command.paused !== 'boolean') throw new Error('Pause needs a boolean')
          paused = command.paused; break
        case 'speed':
          if (!Number.isFinite(command.speed) || command.speed < MIN_SPEED || command.speed > MAX_SPEED) throw new Error('Replay speed must be between 0.1 and 16')
          speed = command.speed; break
        case 'seek':
          if (!Number.isSafeInteger(command.timeMs) || command.timeMs < 0 || command.timeMs > replay.duration) throw new Error('Seek time exceeds the recording')
          for (const viewer of viewers) if (command.timeMs < viewer.target()) throw new Error('Backward seek needs a fresh connection; reset playback at that timestamp')
          startMs = Math.max(command.timeMs, bootstrapTime)
          for (const viewer of viewers) viewer.seek(startMs)
          break
        case 'reset': {
          const timeMs = command.timeMs === undefined ? bootstrapTime : command.timeMs
          if (!Number.isSafeInteger(timeMs) || timeMs < 0 || timeMs > replay.duration) throw new Error('Reset time exceeds the recording')
          if (generation >= 1_000_000_000) throw new Error('Replay generation limit reached')
          if (command.reconnect !== undefined && typeof command.reconnect !== 'boolean') throw new Error('Reset reconnect must be a boolean')
          let previous, transfer
          if (command.reconnect === true) {
            if (config.transport !== 'raknet' || replay.metadata.version !== '1.26.51') throw new Error('Automatic replay reconnect currently requires the local RakNet protocol 1.26.51 listener')
            if (pendingReset) throw new Error('A fresh-world reconnect is already pending')
            if (viewers.size !== 1 || !(previous = viewers.values().next().value).initialized() || !previous.identity()) throw new Error('Automatic reset needs one initialized replay viewer')
            // Serialize before changing generation so a schema failure cannot
            // destroy the current world. Target only this owned loopback listener.
            transfer = server.serializer.createPacketBuffer({ name: 'transfer', params: {
              server_address: config.host, port: config.listenPort, reload_world: true,
              gatherings_configuration: undefined } })
          }
          cancelStep('Replay step canceled by world reset')
          if (pendingReset) failReset('Replay reconnect superseded by manual world reset')
          generation++; lastStepSequence = 0; paused = true; startMs = Math.max(timeMs, bootstrapTime)
          if (previous) {
            const identity = previous.identity()
            pendingReset = { generation, timeMs: startMs, stage: 'awaiting_connection', oldViewerId: identity.viewerId,
              expectedUuid: identity.playerUuid, expectedName: identity.username,
              timeout: setTimeout(() => failReset('Automatic replay reconnect timed out; join the local replay address again'), 60000) }
            try { previous.reconnect(transfer) }
            catch (error) { previous.stop(error.message); failReset(`Replay transfer failed: ${error.message}`); throw error }
          } else {
            for (const viewer of [...viewers]) viewer.stop('Arctic replay restarting from a fresh world')
            lastReset = { generation, timeMs: startMs, stage: 'awaiting_connection', automaticReconnect: false }
          }
          emit({ event: 'replay_reset', generation, positionMs: startMs, connectionRequired: true,
            worldReset: resetState(), nativeWorldVerified: false,
            message: previous ? 'Replay transfer sent; waiting for a fresh connection and world initialization'
              : 'Replay reset is waiting for a fresh game connection', xboxAuth: false })
          break
        }
        case 'step': {
          if (!paused) throw new Error('Exact replay stepping requires paused playback')
          if (command.generation !== generation) throw new Error('Replay step belongs to a different world generation')
          if (!Number.isSafeInteger(command.sequence) || command.sequence <= lastStepSequence || command.sequence > 1_000_000_000) throw new Error('Replay step sequence must increase')
          if (!Number.isSafeInteger(command.timeMs) || command.timeMs < bootstrapTime || command.timeMs > replay.duration) throw new Error('Step time exceeds the recording or precedes its world initialization checkpoint')
          let cameraTime, cameraAt
          if (command.cameraTime !== undefined) {
            const value = command.cameraTime
            if (!value || typeof value !== 'object' || Array.isArray(value)
              || Object.keys(value).some(key => !['remainder', 'denominator'].includes(key))
              || !Number.isSafeInteger(value.denominator) || value.denominator < 1 || value.denominator > 1000
              || !Number.isSafeInteger(value.remainder) || value.remainder < 0 || value.remainder >= value.denominator) throw new Error('Invalid rational export camera time')
            cameraTime = { remainder: value.remainder, denominator: value.denominator }
            cameraAt = camera.sampleTime({ timeMs: command.timeMs, ...cameraTime })
            if (cameraAt > replay.duration) throw new Error('Export camera sample exceeds the recording')
          }
          if (!viewers.size) throw new Error('Replay stepping needs a game connection')
          if (command.marker !== undefined) {
            if (typeof command.marker !== 'string' || !/^37c1[0-9a-f]{12}$/.test(command.marker)) throw new Error('Invalid native replay marker timestamp')
            if (viewers.size !== 1 || ![...viewers].every(v => v.initialized())) throw new Error('Native replay markers need one initialized viewer')
            if (markers.size >= 1_000_000 || markers.has(command.marker)) throw new Error('Native replay marker was already used or its process limit was reached')
          }
          for (const viewer of viewers) if (command.timeMs < viewer.target()) throw new Error('Backward step needs a fresh world reset')
          if (cameraTime) for (const viewer of viewers) if (cameraAt < viewer.cameraTarget()) throw new Error('Backward export camera sample needs a fresh world reset')
          lastStepSequence = command.sequence; startMs = command.timeMs
          if (command.marker !== undefined) markers.add(command.marker)
          const done = new Promise((resolve, reject) => {
            pendingStep = { sequence: command.sequence, timeMs: command.timeMs, marker: command.marker, cameraTime, cameraAt,
              viewer: command.marker === undefined ? null : viewers.values().next().value, resolve, reject,
              timeout: setTimeout(() => cancelStep('Replay packet step timed out'), 30000) }
          })
          for (const viewer of viewers) viewer.seek(command.timeMs, cameraAt)
          const result = await done
          emit(status())
          return result
        }
        case 'camera': case 'camera_path':
          if (replay.metadata.version !== '1.26.51') throw new Error('Camera editing currently requires protocol data 1.26.51 (2193)')
          // Keep one revision available for the owning export's clear.
          if (cameraRevision >= 999_999_999) throw new Error('Camera revision budget exhausted')
          if (command.op === 'camera') { fixedCamera = camera.pose(command.pose); path = null; cameraPathSha256 = null }
          else {
            const accepted = camera.cameraPath(command.path, replay.duration), digest = camera.pathSha256(accepted)
            path = accepted; cameraPathSha256 = digest; fixedCamera = null
          }
          cameraRevision++; break
        case 'camera_clear':
          if (cameraRevision >= 1_000_000_000) throw new Error('Camera revision budget exhausted')
          path = null; fixedCamera = null; cameraPathSha256 = null; cameraRevision++; break
        case 'export': {
          const exportPath = command.path === undefined ? path : camera.cameraPath(command.path, replay.duration)
          if (!exportPath) throw new Error('Create a camera path before exporting an edited replay')
          if (exports.size) throw new Error('A replay export is already running')
          const job = exportCameraReplay(config.file, command.file, exportPath, { fps: command.fps }); exports.add(job)
          try { await job; emit({ event: 'replay_exported', message: command.file }) } finally { exports.delete(job) }
          break
        }
      }
      for (const viewer of viewers) viewer.resetClock()
      const state = status(); emit(state); return state
    }
    server.control = async command => {
      // Legacy commands keep their existing event format. Export workers opt
      // into a distinct, non-coalesced reply over the same owned pipe.
      if (!command || typeof command !== 'object' || Array.isArray(command)) throw new Error('Invalid replay control')
      if (command.requestId === undefined) return applyControl(command)
      if (typeof command.requestId !== 'string' || !/^[A-Za-z0-9_-]{1,64}$/.test(command.requestId)) throw new Error('Invalid replay control request identity')
      const { requestId, ...operation } = command
      const result = await applyControl(operation)
      emit({ event: 'replay_control', requestId, operation: operation.op, result,
        state: { ...status(), lastStepSequence, viewerTargets: [...viewers].map(viewer => ({
          viewerId: viewer.identity()?.viewerId, packetTimeMs: viewer.target(), cameraTimeMs: viewer.cameraTarget() })) } })
      return result
    }
    server.on('connect', player => {
      bindOfflinePeer(server, player)
      nethernet?.authenticate(player)
      if (viewers.size) { player.close('This local replay already has an active viewer'); return }
      const viewerGeneration = generation
      let timer, cursor = 0, elapsed = startMs, previous = performance.now(), waiting = null, waitAt = 0, spawnAt = 0, selfId
      let applied = 0, finished = false, lastStatus = 0, lastCamera = 0, revision = -1, activeCamera = false
      let preset = camera.presets(), cameraDirty = true, caughtUp = false, stopped = false
      let cameraTime = null, cameraQueuedTime = null, cameraQueuedPose = null
      let initialized = false
      let bootstrapComplete = !config.waitForClientInit, initializationStarted = null
      const viewerId = randomUUID()
      const packs = resourcePacks.session(player)
      const self = new ReplaySelf(replay.metadata, player, offlineUuid(replay.metadata.username), originalRuntime, original.entity_id)
      const packFailure = error => {
        if (stopped || viewerGeneration !== generation) return
        cancelStep(`Replay resource-pack negotiation failed: ${error.message}`)
        viewer.stop(error.message)
        emit({ event: 'error', message: error.message }); emit(status())
      }
      const viewer = { position: () => applied, target: () => elapsed, ready: () => caughtUp && !stopped,
        cameraTarget: () => cameraTime ?? elapsed, cameraPosition: () => cameraQueuedTime,
        cameraPose: () => cameraQueuedPose,
        marker: timestamp => {
          if (stopped || !initialized || !caughtUp) throw new Error('Replay viewer is not ready for its native marker')
          // New connection-local probe, after world/camera packets in the same
          // queue. Recorded latency probes remain filtered from playback.
          player.queue('network_stack_latency', { timestamp: BigInt(`0x${timestamp}`), needs_response: true })
        },
        initialized: () => initialized && !stopped,
        identity: () => player.profile ? { viewerId, playerUuid: player.profile.uuid, username: player.profile.name,
          generation: viewerGeneration, clientInitialized: initialized && !stopped } : null,
        seek: (time, sampleTime) => { elapsed = time; cameraTime = sampleTime ?? null; cameraQueuedTime = null; cameraQueuedPose = null;
          finished = false; caughtUp = false; lastCamera = -Infinity },
        resetClock: () => { previous = performance.now() },
        reconnect: transfer => {
          if (stopped || !initialized || viewerGeneration + 1 !== generation) throw new Error('Replay transfer lost its original initialized viewer')
          stopped = true; clearInterval(timer); timers.delete(timer); viewers.delete(viewer)
          // Flush queued old-world packets, then send transfer in protocol order.
          // Keep transport alive briefly; an immediate disconnect can discard it.
          player.onTick(); player.sendBuffer(transfer, true)
          const deadline = setTimeout(() => { retiring.delete(player); player.close('Replay transfer peer retired') }, 3000)
          retiring.set(player, deadline)
        },
        stop: message => { stopped = true; clearInterval(timer); timers.delete(timer); viewers.delete(viewer); player.disconnect(message) } }
      viewers.add(viewer)
      player.on('error', error => { if (!stopped && viewerGeneration === generation) emit({ event: 'error', generation: viewerGeneration, message: error.message }) })
      player.on('set_local_player_as_initialized', packet => {
        if (stopped || viewerGeneration !== generation) return
        const runtimeId = Number(packet.runtime_entity_id)
        if (selfId == null || !Number.isSafeInteger(runtimeId) || runtimeId !== selfId) {
          cancelStep('Replay client initialized an unexpected player')
          viewer.stop('Replay client initialized an unexpected player')
          emit({ event: 'error', message: 'Replay client initialized an unexpected player', viewerId,
            generation: viewerGeneration, xboxAuth: false })
          return
        }
        if (initialized) return
        initialized = true
        previous = performance.now()
        // The pinned library's join event means encryption completed. This
        // separate client packet acknowledges its local world initialization;
        // it is still not a packet-application, terrain or rendering fence.
        emit({ event: 'replay_initialized', viewerId, playerUuid: player.profile.uuid,
          username: player.profile.name, generation: viewerGeneration,
          clientInitialized: true, nativeWorldVerified: false, xboxAuth: false })
        emit(status())
      })
      player.on('resource_pack_client_response', packet => {
        if (stopped || viewerGeneration !== generation) return
        try {
          if (packs.response(packet)) waiting = null
          waitAt = previous = performance.now()
        } catch (error) { packFailure(error) }
      })
      player.on('resource_pack_chunk_request', packet => {
        if (stopped || viewerGeneration !== generation) return
        try { packs.request(packet) } catch (error) { packFailure(error) }
      })
      player.once('join', () => {
        if (stopped || viewerGeneration !== generation) return
        if (pendingReset) {
          if (pendingReset.generation !== viewerGeneration || pendingReset.newViewerId
              || player.profile.uuid !== pendingReset.expectedUuid || player.profile.name !== pendingReset.expectedName
              || viewerId === pendingReset.oldViewerId) {
            viewer.stop('Fresh replay connection does not match the requested reset viewer'); return
          }
          pendingReset.newViewerId = viewerId; pendingReset.stage = 'initializing'
        }
        emit({ event: 'replay_joined', viewerId, playerUuid: player.profile.uuid,
          username: player.profile.name, generation, clientInitialized: false, xboxAuth: false })
        timer = setInterval(() => {
          try {
            if (stopped || viewerGeneration !== generation) return
            const now = performance.now()
            if(now<spawnAt) {previous=now;return}
            if (waiting) {
              if (packs.drain()) waitAt = now
              if (now - waitAt > 30000) throw new Error('Replay resource-pack handshake timed out')
              previous = now; return
            }
            if (!bootstrapComplete && initializationStarted !== null && now - initializationStarted > 30000) throw new Error('Replay client world initialization timed out')
            if (!paused) cameraTime = null
            if (!paused && bootstrapComplete) elapsed = Math.min(replay.duration,
              elapsed + Math.min(MAX_FRAME_MS, Math.max(0, now - previous)) * speed)
            previous = now
            let bytes = 0, count = 0
            // Feed the recorded bootstrap in bounded batches, including its
            // client checkpoint, then wait without consuming later packets or
            // accumulating wall time. Paused playback must bootstrap too.
            while (cursor < replay.records.length && (bootstrapComplete ? replay.records[cursor].time <= elapsed : cursor <= initializedIndex) && count < 128 && bytes < 256 * 1024) {
              const record = replay.records[cursor++]
              const buffer = replay.packet(record)
              const { name, params } = server.deserializer.parsePacketBuffer(buffer).data
              bytes += buffer.length; count++
              applied = record.time
              if (record.direction === 1) {
                // The server does not echo a player's own movement. Translate
                // movement and the owner's visual state into server packets;
                // arbitrary client requests are never sent back to the game.
                if (selfId != null && (name === 'player_auth_input' || name === 'move_player')) {
                  const position=params.position
                  if(!position || ![position.x,position.y,position.z,params.pitch,params.yaw,params.head_yaw].every(Number.isFinite)) throw new Error('Invalid recorded player movement')
                  player.queue('move_player',{runtime_id:selfId,position,pitch:params.pitch,yaw:params.yaw,head_yaw:params.head_yaw,
                    mode:'teleport',on_ground:params.on_ground===true,ridden_runtime_id:0,tick:params.tick??0n})
                }
                self.client(name, params)
                continue
              }
              if (AUTH.has(name) || CONNECTION_CONTROL.has(name) || name === 'disconnect' || name === 'play_status' && params.status === 'login_success') continue
              // The fresh viewer asks for its own chunk order. Historical pack
              // transfer packets are indexed archives, not world-timed traffic.
              if (TRANSFER_PACKETS.has(name)) continue
              if ((path || fixedCamera) && camera.CAMERA_PACKETS.has(name)) continue
              if (name === 'camera_presets') {
                preset = camera.presets(params.presets); cameraDirty = true
              }
              if (name === 'resource_packs_info') {
                player.queue(name, packs.offer(params))
                waiting = 'have_all_packs'; waitAt = now
                break
              } else if (name === 'resource_pack_stack') {
                packs.stack(params)
                waiting = 'completed'; waitAt = now
              }
              if(name==='start_game') {
                selfId=Number(params.runtime_entity_id)
                if(!Number.isSafeInteger(selfId)||selfId<0||selfId>0xFFFFFFFF) throw new Error('Recorded player runtime id exceeds this replay protocol adapter')
                self.bind(selfId)
                spawnAt=config.waitForClientInit ? 0 : now+500
                initializationStarted=now
              }
              const projected = self.server(name, params)
              if (projected === params) {
                player._processOutbound(name, params) // Restore item palette before sending raw world packets.
                player.sendBuffer(buffer)
              } else if (projected !== null) player.queue(name, projected)
              if (name === 'start_game') self.start()
              self.flushSkin()
              if (waiting || spawnAt>now) break
            }
            if (!bootstrapComplete && initialized && cursor > initializedIndex) {
              bootstrapComplete = true; previous = now
            }
            caughtUp = bootstrapComplete && !waiting && now >= spawnAt && selfId != null && (cursor === replay.records.length || replay.records[cursor].time > elapsed)
            if (caughtUp) {
              applied = Math.floor(elapsed)
              if ((path || fixedCamera) && (revision !== cameraRevision || cameraDirty || (paused ? lastCamera === -Infinity : now - lastCamera >= 40))) {
                if (cameraDirty) { player.queue('camera_presets', { presets: preset.presets }); cameraDirty = false }
                // Packet receipts expose whole milliseconds, but Java's live
                // CameraPath consumes its double playhead. Preserve the
                // fractional normal-playback time as well as explicit export
                // remainders instead of quantizing the camera to applied.
                const sampleTime = cameraTime ?? elapsed
                const target = camera.wirePose(fixedCamera ?? camera.sampleForView(path, sampleTime))
                player.queue('camera_instruction', camera.instruction(target, preset.id))
                cameraQueuedTime = sampleTime
                cameraQueuedPose = target
                lastCamera = now; activeCamera = true; revision = cameraRevision
              } else if (!path && !fixedCamera && activeCamera) {
                player.queue('camera_instruction', { clear: true, fov: { field_of_view: 70, ease_time: 0, ease_type: 'linear', clear: true } })
                activeCamera = false; revision = cameraRevision
                cameraQueuedTime = null
                cameraQueuedPose = null
              }
            }
            completeStep()
            advanceReset(viewer)
            if (now - lastStatus >= 250) { emit(status()); lastStatus = now }
            if (!finished && caughtUp && cursor === replay.records.length && elapsed >= replay.duration) {
              finished = true
              emit({ event: 'replay_finished', message: 'Recording reached the end; the last world state stays visible' })
            }
          } catch (error) {
            cancelStep(`Replay packet step failed: ${error.message}`)
            viewer.stop(error.message)
            emit({ event: 'error', message: error.message }); emit(status())
          }
        }, 10)
        timers.add(timer)
      })
      player.once('close', () => {
        const deadline = retiring.get(player)
        if (deadline) { clearTimeout(deadline); retiring.delete(player) }
        stopped = true; clearInterval(timer); timers.delete(timer); viewers.delete(viewer)
        if (viewerGeneration === generation) cancelStep('Replay viewer disconnected during packet step')
        if (pendingReset?.newViewerId === viewerId) failReset('Fresh replay viewer disconnected before reset completed')
        emit({ event: 'replay_disconnected', generation: viewerGeneration, xboxAuth: false })
        emit(status())
      })
    })
    server.on('error', error => emit({ event: 'error', message: error.message }))
    const close = server.close.bind(server)
    server.close = async (...args) => {
      closed = true
      if (pendingReset) { clearTimeout(pendingReset.timeout); pendingReset = null }
      for (const [player, deadline] of retiring) { clearTimeout(deadline); player.close('Replay stopped during transfer') }
      retiring.clear()
      nethernet?.close()
      cancelStep('Replay stopped during packet step')
      for (const timer of timers) clearInterval(timer)
      timers.clear()
      for (const player of Object.values(server.clients)) player.close('Replay stopped')
      try { await Promise.allSettled([...exports]); await close(...args) } finally { replay.close() }
    }
    await server.listen()
    nethernet?.start()
    emit({ event: 'ready', address: `${config.host}:${config.listenPort}`, username: replay.metadata.username,
      version: replay.metadata.version, transport: config.transport, xboxAuth: false })
    return server
  } catch (error) { nethernet?.close(); if (server) await server.close(); replay.close(); throw error }
}

module.exports = { PacketRecording, PacketReplay, startPlayback, validatePlayback, exportCameraReplay, AUTH }
