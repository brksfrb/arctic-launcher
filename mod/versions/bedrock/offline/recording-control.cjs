'use strict'

function timestampHex(value) {
  if (typeof value === 'bigint') return BigInt.asUintN(64, value).toString(16).padStart(16, '0')
  // ProtoDef's li64 decoder uses SignedBigInt([high, low]); both words can
  // carry an Int32 sign. Match raw uint64 bits, not an imprecise Number.
  if (Array.isArray(value) && value.length === 2 && value.every(n => Number.isInteger(n) && n >= -0x80000000 && n <= 0xffffffff)) {
    return BigInt.asUintN(64, (BigInt(value[0]) << 32n) | BigInt.asUintN(32, BigInt(value[1]))).toString(16).padStart(16, '0')
  }
  return null
}

// Correlated control for an existing unsigned relay recording. A marker queues
// locally after its timestamp is recorded; it never uses the upstream server.
class RecordingControl {
  constructor(relay, emit, onFinish) { this.relay = relay; this.emit = emit; this.onFinish = onFinish; this.entries = new Set(); this.used = new Set() }
  attach(player, recording, file) {
    const entry = { player, recording, file, initialized: false, connected: true, sequence: 0 }
    this.entries.add(entry)
    player.on('serverbound', ({ name, params }, packet) => {
      if (name === 'set_local_player_as_initialized') entry.initialized = true
      if (name === 'network_stack_latency' && this.used.has(timestampHex(params.timestamp))) packet.canceled = true
    })
    player.once('close', () => { entry.connected = false; entry.initialized = false })
    return entry
  }
  state() {
    const entries = [...this.entries].filter(e => !e.recording.ended)
    const entry = entries.length === 1 ? entries[0] : null
    return { event: 'recording_state', recordings: entries.length, recordingId: entry?.recording.metadata.recordingId,
      file: entry?.file, positionMs: entry?.recording.position(), initialized: entry?.initialized === true && entry.connected,
      failed: !!entry?.recording.error || !!entry && entry.recording.stream.destroyed,
      generation: 1, sequence: entry?.sequence, xboxAuth: false, nativeWorldVerified: false }
  }
  async control(command) {
    if (!command || typeof command !== 'object' || Array.isArray(command)
      || typeof command.requestId !== 'string' || !/^[A-Za-z0-9_-]{1,64}$/.test(command.requestId)) throw new Error('Invalid recording control identity')
    const { requestId, ...op } = command
    let result
    if (op.op === 'recording_state' && Object.keys(op).length === 1) result = this.state()
    else {
      const entry = [...this.entries].find(e => e.recording.metadata.recordingId === op.recordingId)
      if (!entry || entry.recording.error) throw new Error('Recording identity is absent or failed')
      if (op.op === 'recording_marker') {
        if (Object.keys(op).some(k => !['op', 'recordingId', 'generation', 'sequence', 'marker'].includes(k))
          || op.generation !== 1 || !Number.isSafeInteger(op.sequence) || op.sequence <= entry.sequence || op.sequence > 1e9
          || typeof op.marker !== 'string' || !/^37c1[0-9a-f]{12}$/.test(op.marker)
          || this.used.has(op.marker) || this.used.size >= 1_000_000) throw new Error('Invalid recording marker or sequence')
        if (!entry.connected || !entry.initialized || entry.recording.ended || this.state().recordings !== 1
          || entry.recording.metadata.version !== '1.26.51') throw new Error('Recording marker needs one initialized protocol-2193 viewer')
        const params = { timestamp: BigInt(`0x${op.marker}`), needs_response: true }
        const bytes = this.relay.serializer.createPacketBuffer({ name: 'network_stack_latency', params })
        const timeMs = entry.recording.position()
        entry.recording.packetAt(timeMs, 0, 'network_stack_latency', bytes)
        if (entry.recording.error || entry.recording.stream.destroyed) throw new Error('Recording failed before its marker could be queued')
        this.used.add(op.marker); entry.sequence = op.sequence
        entry.player.queue('network_stack_latency', params)
        result = { recordingId: op.recordingId, generation: 1, sequence: op.sequence, marker: op.marker, timeMs,
          markerQueued: true, nativeWorldVerified: false, packetApplicationVerified: false }
      } else if (op.op === 'recording_finish' && Object.keys(op).length === 2) {
        await entry.recording.finish()
        this.onFinish?.(entry.recording)
        result = { recordingId: op.recordingId, file: entry.file, durationMs: entry.recording.duration,
          packets: entry.recording.count, bytes: entry.recording.bytes, sourceSha256: entry.recording.digest(), finalized: true }
      } else throw new Error('Unsupported recording control or fields')
    }
    this.emit({ event: 'recording_control', requestId, operation: op.op, result, state: this.state() })
    return result
  }
}

module.exports = { RecordingControl }
