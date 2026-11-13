'use strict'

const { isIP } = require('node:net')
const { parseArgs } = require('node:util')
const { readFileSync } = require('node:fs')
const { createHash, randomUUID } = require('node:crypto')
const { isAbsolute } = require('node:path')
const { PacketRecording } = require('./replay.cjs')
const { RecordingControl } = require('./recording-control.cjs')

const PREFIX = 'ARCTIC_BEDROCK_OFFLINE '
let protocol

function offlineUuid(name) {
  const bytes = createHash('md5').update(`OfflinePlayer:${name}`, 'utf8').digest()
  bytes[6] = (bytes[6] & 15) | 48
  bytes[8] = (bytes[8] & 63) | 128
  const hex = bytes.toString('hex')
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`
}

function validateAppearance(value) {
  if (value == null) return null
  if (typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).some(k => !['skin', 'cape', 'slim'].includes(k))) throw new Error('Invalid offline appearance')
  if (value.slim !== undefined && typeof value.slim !== 'boolean') throw new Error('Invalid slim skin option')
  const image = (source, cape) => {
    if (source == null) return null
    if (typeof source !== 'object' || Object.keys(source).some(k => !['width', 'height', 'data'].includes(k))) {
      throw new Error('Invalid appearance image')
    }
    const { width, height, data } = source
    if (width !== 64 || !(cape ? height === 32 : [32, 64].includes(height))) throw new Error('Invalid appearance dimensions')
    if (typeof data !== 'string' || data.length !== Math.ceil(width * height * 4 / 3) * 4) {
      throw new Error('Invalid appearance pixel buffer')
    }
    const bytes = Buffer.from(data, 'base64')
    if (bytes.length !== width * height * 4 || bytes.toString('base64') !== data) throw new Error('Invalid appearance pixels')
    return { width, height, data }
  }
  const skin = image(value.skin, false), cape = image(value.cape, true)
  if (!skin && (!cape || value.slim)) throw new Error('A custom skin is required for slim arms')
  return { skin, cape, slim: value.slim === true }
}

function appearancePayload(appearance) {
  if (!appearance) return {}
  const { skin, cape, slim } = appearance
  const patch = {}
  if (skin) {
    Object.assign(patch, {
      SkinId: `arctic-${createHash('sha256').update(skin.data).digest('hex')}`,
      SkinData: skin.data, SkinImageWidth: skin.width, SkinImageHeight: skin.height,
      SkinResourcePatch: Buffer.from(JSON.stringify({ geometry: {
        default: slim ? 'geometry.humanoid.customSlim' : 'geometry.humanoid.custom'
      } })).toString('base64'),
      SkinGeometryData: Buffer.from('null').toString('base64'),
      SkinGeometryDataEngineVersion: Buffer.from('1.12.0').toString('base64'),
      ArmSize: slim ? 'slim' : 'wide', PersonaSkin: false, PremiumSkin: false,
      PersonaPieces: [], PieceTintColors: [], AnimatedImageData: [], SkinAnimationData: '',
      TrustedSkin: false, OverrideSkin: true
    })
  }
  if (cape) Object.assign(patch, {
    CapeId: `arctic-${createHash('sha256').update(cape.data).digest('hex')}`,
    CapeData: cape.data, CapeImageWidth: cape.width, CapeImageHeight: cape.height, CapeOnClassicSkin: true
  })
  return patch
}

function validateConfig(input) {
  if (!input || typeof input !== 'object') throw new Error('An offline connector configuration is required')
  const allowed = new Set(['host', 'port', 'username', 'version', 'listenPort', 'appearance', 'recordTo'])
  for (const key of Object.keys(input)) {
    if (!allowed.has(key)) throw new Error(`Unsupported offline connector option: ${key}`)
  }
  const host = String(input.host ?? '').trim()
  const username = String(input.username ?? '').trim()
  if (!host || host.length > 253 || /[\s/\\@?#]/u.test(host)) throw new Error('Enter a server hostname or IP address')
  if (!isIP(host) && !/^[a-z\d](?:[a-z\d.-]*[a-z\d])?$/iu.test(host)) throw new Error('Invalid server hostname')
  if (isIP(host) === 6) throw new Error('The offline connector currently needs an IPv4 address or hostname')
  // Offline names are deliberately conservative and remain stable across reconnects.
  if (!/^[A-Za-z0-9_]{3,16}$/u.test(username)) throw new Error('Use an offline name with 3–16 letters, digits or underscores')
  const port = input.port ?? 19132
  const listenPort = input.listenPort ?? 19142
  for (const value of [port, listenPort]) {
    if (!Number.isInteger(value) || value < 1 || value > 65535) throw new Error('Ports must be between 1 and 65535')
  }
  if (['127.0.0.1', 'localhost', '::1'].includes(host.toLowerCase()) && port === listenPort) {
    throw new Error('The offline connector cannot forward to itself')
  }
  const version = input.version == null ? null : String(input.version)
  if (version !== null && !/^\d+\.\d+\.\d+(?:\.\d+)?$/u.test(version)) throw new Error('Invalid Bedrock protocol version')
  const appearance = validateAppearance(input.appearance)
  if (input.recordTo != null && (typeof input.recordTo !== 'string' || !isAbsolute(input.recordTo))) throw new Error('Replay output must be an absolute path')
  return { host, port, listenPort, username, version, ...(appearance ? { appearance } : {}), ...(input.recordTo ? { recordTo: input.recordTo } : {}) }
}

function loadProtocol() {
  if (protocol) return protocol
  require('./camera.cjs').patchCameraSchema()
  // These functions are never needed for an unsigned session. Block them in this
  // private helper process so an accidental future fallback cannot prompt for an
  // account, read cached Xbox credentials, or request an online token.
  const auth = require('bedrock-protocol/src/client/auth')
  for (const method of ['authenticate', 'realmAuthenticate', 'worldAuthenticate', 'serverAuthenticate']) {
    auth[method] = () => { throw new Error('Xbox authentication is disabled in Arctic offline mode') }
  }
  const createOfflineSession = auth.createOfflineSession
  auth.createOfflineSession = (client, options) => {
    if (options.offline !== true) throw new Error('Xbox authentication is disabled in Arctic offline mode')
    // Keep identities consistent with Java offline accounts and Arctic's skin
    // library. Run before the protocol's own session listeners sign the login.
    client.prependOnceListener('session', profile => { profile.uuid = offlineUuid(options.username) })
    return createOfflineSession(client, options)
  }
  const options = require('bedrock-protocol/src/options')
  // Relay 3.60.1 does not forward raknetBackend to its upstream Client. Set the
  // private process defaults as well as the instance options (no native addon).
  options.defaultOptions.raknetBackend = 'jsp-raknet'
  options.defaultOptions.useRaknetWorkers = false
  options.defaultOptions.offline = true
  const { Server } = require('jsp-raknet')
  const sendBuffer = Server.prototype.sendBuffer
  // jsp-raknet 2.1.3 schedules peer closes after closing its UDP socket. Do not
  // write final disconnect datagrams once the transport has stopped. Errors on
  // a running transport still propagate normally.
  Server.prototype.sendBuffer = function (buffer, client) {
    if (this.running) return sendBuffer.call(this, buffer, client)
  }
  protocol = require('bedrock-protocol')
  return protocol
}

function bindOfflinePeer(server, player) {
  if (server.options.transport === 'nethernet') {
    const peer = player.connection, owner = server.transport?.nethernet
    if (peer?.nethernet !== owner || !(owner?.connections instanceof Map)
        || owner.connections.get(peer.address) !== peer) throw new Error('Unsupported owned NetherNet peer')
    // Release only this viewer's slot. A delayed close from an older RTC peer
    // must not retire a replacement connection with the same address.
    player.once('close', () => {
      if (server.clients[peer.address] === player) server.onCloseConnection(peer, 'Owned local replay peer closed')
    })
    return
  }
  const peer = player.connection, transport = server.transport?.raknet
  if (server.options.raknetBackend !== 'jsp-raknet' || peer?.listener !== transport
      || transport?.server !== true || !(transport.connections instanceof Map)) {
    throw new Error('Unsupported owned offline peer transport')
  }
  // jsp-raknet 2.1.3 closes the listener when *one* peer receives a disconnect,
  // and local peer.close does not emit the server's closeConnection callback.
  // Scope the correction to this owned peer, preserving the client-side socket
  // behavior. Compare both identities before releasing a slot or map entry so
  // a delayed close cannot retire a replacement connection at the same address.
  const retire = () => {
    if (server.clients[peer.address] === player) server.onCloseConnection(peer, 'Owned offline peer closed')
    if (transport.connections.get(peer.address.hash) === peer) transport.connections.delete(peer.address.hash)
  }
  peer.disconnect = reason => { player.close(reason); retire() }
  player.once('close', retire)
}

async function startConnector(input, emit = () => {}) {
  const config = validateConfig(input)
  const bedrock = loadProtocol()
  const { Versions } = require('bedrock-protocol/src/options')
  let version = config.version
  if (version && !Versions[version]) throw new Error(`The offline connector does not support Bedrock ${version}`)
  if (!version) {
    const pong = await bedrock.ping({ host: config.host, port: config.port, timeout: 2500, raknetBackend: 'jsp-raknet' })
    version = Object.keys(Versions).find(v => Versions[v] === Number(pong.protocol))
    if (!version) throw new Error(`The server uses an unsupported Bedrock protocol (${pong.protocol})`)
  }
  const options = {
    // Local-only endpoint: this is not a public unauthenticated proxy.
    host: '127.0.0.1',
    port: config.listenPort,
    version,
    offline: true,
    transport: 'raknet',
    raknetBackend: 'jsp-raknet',
    useRaknetWorkers: false,
    forceSingle: true,
    maxPlayers: 1,
    logging: false,
    motd: { motd: `Arctic Offline · ${config.username}`, levelName: config.host },
    destination: { host: config.host, port: config.port, offline: true, transport: 'raknet' },
    onMsaCode: () => { throw new Error('Xbox authentication is disabled in Arctic offline mode') }
  }
  class OfflineRelay extends bedrock.Relay {
    async close(reason) {
      // In 3.60.1 Server.close closes the transport at the same deadline as
      // delayed peer disconnects. Close peers first while their socket is live.
      for (const player of Object.values(this.clients)) player.close(reason)
      return super.close(reason)
    }

    async openUpstreamConnection(player, address) {
      if (this.options.offline !== true || this.options.destination.offline !== true) {
        throw new Error('Refusing an authenticated connection in offline mode')
      }
      player.profile = { ...player.profile, name: config.username, xuid: '' }
      if (player.skinData) {
        player.skinData = { ...player.skinData, ...appearancePayload(config.appearance), ThirdPartyName: config.username, ThirdPartyNameOnly: true,
          PlayFabId: '', PlatformOnlineId: '' }
      }
      return super.openUpstreamConnection(player, address)
    }
  }
  const relay = new OfflineRelay(options)
  const recordingControl = new RecordingControl(relay, emit, recording => finish(recording))
  let controls = Promise.resolve()
  relay.control = command => {
    const operation = controls.then(() => recordingControl.control(command))
    controls = operation.catch(() => {})
    return operation
  }
  const recordings = new Set(), finalizing = new Set()
  const report = error => emit({ event: 'error', message: `Replay recording: ${error.message}` })
  const finish = recording => {
    if (!recordings.delete(recording)) return
    const pending = recording.finish().then(() => emit({ event: 'recording_saved', message: config.recordTo }), report)
      .finally(() => finalizing.delete(pending))
    finalizing.add(pending)
  }
  const closeRelay = relay.close.bind(relay)
  relay.close = async (...args) => {
    try { await closeRelay(...args) } finally {
      for (const recording of [...recordings]) finish(recording)
      await Promise.all(finalizing)
    }
  }
  relay.on('error', error => emit({ event: 'error', message: error.message }))
  relay.on('connect', player => {
    bindOfflinePeer(relay, player)
    // Apply policy before recording, so canceled appearance updates cannot
    // replace the selected skin during playback.
    player.on('serverbound', ({ name, params }, packet) => {
      if (name !== 'player_skin') return
      if (config.appearance) { packet.canceled = true; return }
      const uuid = String(params.uuid ?? '').toLowerCase(), own = offlineUuid(config.username)
      const incoming = player.profile?.uuid?.toLowerCase()
      // The game and the unsigned upstream session can have different UUIDs.
      // Carry only this connection owner's update into its upstream identity.
      if (uuid !== own && uuid !== incoming) { packet.canceled = true; return }
      params.uuid = own
    })
    if (config.recordTo) {
      try {
        const recording = new PacketRecording(config.recordTo, { version, username: config.username,
          uuid: offlineUuid(config.username), recordingId: randomUUID(), createdAt: new Date().toISOString() }, report)
        recordings.add(recording)
        recordingControl.attach(player, recording, config.recordTo)
        // Auth envelopes never reach this recorder.
        player.on('clientbound', ({ name }, packet) => {
          if (!packet.canceled) recording.packet(0, name, packet.fullBuffer)
        })
        player.on('serverbound', ({ name, params }, packet) => {
          if (packet.canceled) return
          try {
            // Skin policy may have remapped UUID above. Record exactly the
            // normalized visual update that the relay actually queues upstream.
            const buffer = name === 'player_skin' ? relay.serializer.createPacketBuffer({ name, params }) : packet.fullBuffer
            recording.packet(1, name, buffer)
          } catch (error) { recording.stream.destroy(error) }
        })
        player.once('close', () => finish(recording))
        emit({ event: 'recording_started', message: config.recordTo, recordingId: recording.metadata.recordingId })
      } catch (error) { report(error) }
    }
    player.on('error', error => emit({ event: 'error', message: error.message }))
    player.on('clientbound', ({ name, params }) => {
      if (name === 'disconnect') emit({ event: 'disconnected', message: params.message || 'The server closed the connection' })
    })
    player.on('close', () => emit({ event: 'disconnected', message: 'Game disconnected' }))
  })
  relay.on('join', (_player, upstream) => {
    if (upstream.options.offline !== true || upstream.profile?.xuid) {
      upstream.close('Arctic offline identity validation failed')
      emit({ event: 'error', message: 'The upstream session was not an offline identity' })
      return
    }
    emit({ event: 'joined', username: config.username, xboxAuth: false })
  })
  await relay.listen()
  emit({ event: 'ready', address: `127.0.0.1:${config.listenPort}`, username: config.username,
    version, xboxAuth: false })
  return relay
}

async function main() {
  const { values } = parseArgs({ options: {
    config: { type: 'string' }, host: { type: 'string' }, port: { type: 'string' },
    username: { type: 'string' }, version: { type: 'string' }, 'listen-port': { type: 'string' },
    'record-to': { type: 'string' }, 'replay-config': { type: 'string' }
  } })
  const config = values.config ? JSON.parse(readFileSync(values.config, 'utf8')) : {
    host: values.host, port: values.port ? Number(values.port) : undefined, username: values.username,
    version: values.version, listenPort: values['listen-port'] ? Number(values['listen-port']) : undefined,
    recordTo: values['record-to']
  }
  // If its owner is killed, stdin closes and we still finalize the recording.
  // A closed status pipe must not turn that cleanup into an uncaught EPIPE.
  let outputClosed = false
  process.stdout.on('error', () => { outputClosed = true })
  const emit = event => { if (!outputClosed) process.stdout.write(PREFIX + JSON.stringify(event) + '\n') }
  const relay = values['replay-config']
    ? await require('./replay.cjs').startPlayback(JSON.parse(readFileSync(values['replay-config'], 'utf8')), emit)
    : await startConnector(config, emit)
  let closing = false
  const close = async () => {
    if (closing) return
    closing = true
    await relay.close('Arctic offline connector stopped')
    process.exit(0)
  }
  process.on('SIGTERM', close)
  process.on('SIGINT', close)
  // The launcher holds stdin open for the session. Its exit releases the helper.
  // Newline-delimited local commands share the owned stdin pipe. Bound lines
  // before JSON parsing; a broken/closed owner still triggers normal cleanup.
  let input = ''
  process.stdin.setEncoding('utf8')
  process.stdin.on('data', chunk => {
    input += chunk
    if (Buffer.byteLength(input) > 1024 * 1024) {
      emit({ event: 'control_error', message: 'Replay command exceeds 1 MiB' }); input = ''; return
    }
    let newline
    while ((newline = input.indexOf('\n')) >= 0) {
      const line = input.slice(0, newline).trim(); input = input.slice(newline + 1)
      if (!line) continue
      try {
        if (!relay.control) throw new Error('This local session has no control consumer')
        const command = JSON.parse(line)
        Promise.resolve(relay.control(command)).catch(error => emit({ event: 'control_error', message: error.message,
          requestId: typeof command?.requestId === 'string' && /^[A-Za-z0-9_-]{1,64}$/.test(command.requestId) ? command.requestId : undefined,
          generation: Number.isSafeInteger(command?.generation) && command.generation >= 0 ? command.generation : undefined,
          sequence: Number.isSafeInteger(command?.sequence) && command.sequence >= 0 ? command.sequence : undefined }))
      } catch (error) { emit({ event: 'control_error', message: error.message }) }
    }
  })
  process.stdin.on('end', close)
}

module.exports = { startConnector, validateConfig, loadProtocol, offlineUuid, bindOfflinePeer, PREFIX }

if (require.main === module) {
  main().catch(error => {
    process.stdout.write(PREFIX + JSON.stringify({ event: 'error', message: error.message }) + '\n')
    process.exit(1)
  })
}

