'use strict'
// Invoked only by the isolated installed-game controller after kernel UDP
// ownership and the native world session have been observed. No fixture server.
const fs = require('node:fs')
const path = require('node:path')
const { createHash } = require('node:crypto')
const { isIP } = require('node:net')
const { spawnSync } = require('node:child_process')
const JWT = require('jsonwebtoken')
const { configureLoopbackDiscovery } = require('./lan-transport.cjs')
const { loadProtocol, offlineUuid } = require('./connector.cjs')
const { PacketRecording, AUTH } = require('./replay.cjs')
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
const loopbackAddress = address => {
  return address === '::1' || (isIP(address || '') === 4 && address.startsWith('127.'))
    || (isIP(address || '') === 6 && address.startsWith('::ffff:127.'))
}
const loopbackCandidate = candidate => loopbackAddress(String(candidate).split(' ')[4])
function verifyOfflineIdentity(client, username) {
  if (client.options.offline !== true || client.accessToken.length || client.profile.xuid !== 0
      || client.profile.uuid !== offlineUuid(username)) throw new Error('Recorder did not retain its offline profile')
  const claims = JWT.verify(client.multiplayerToken, client.ecdhKeyPair.publicKey,
    { algorithms: ['ES384'], issuer: 'self', audience: 'api://auth-minecraft-services/multiplayer' })
  if (claims.cpk !== client.clientX509 || claims.xid !== '' || claims.xname !== username
      || claims.leguuid !== client.profile.uuid || claims.mid !== '') {
    throw new Error('Recorder identity is not its own locally self-signed offline identity')
  }
}

async function record(root) {
  root = fs.realpathSync(root)
  const source = JSON.parse(fs.readFileSync(path.join(root, 'lan-record-source.json'), 'utf8'))
  const config = JSON.parse(fs.readFileSync(path.join(root, 'arctic-offline-probe.json'), 'utf8').replace(/^\uFEFF/, ''))
  if (source.schema !== 1 || !['127.0.0.1', '::1'].includes(source.address)
      || (source.transport === 'raknet' && source.address !== '127.0.0.1')
      || !({ nethernet: 7551, raknet: 19132 }[source.transport] === source.port)
      || !Number.isInteger(source.pid) || source.pid < 1 || !/^native_\d+_\d+$/.test(source.worldSession)
      || !source.worldSession.startsWith(`native_${source.pid}_`) || source.kernelApi !== 'GetExtendedUdpTable'
      || !Number.isInteger(source.attempt) || source.attempt < 1 || source.attempt > 20
      || source.workerSha256 !== createHash('sha256').update(fs.readFileSync(__filename)).update('\0')
        .update(fs.readFileSync(path.join(__dirname, 'lan-transport.cjs'))).digest('hex')
      || Date.now() - source.observedAtMs < 0 || Date.now() - source.observedAtMs > 5000
      || config.lan_record_probe !== true || config.empty_user_provider !== true
      || config.mode !== 'loopback-and-deny-user-provider') throw new Error('Missing fresh owned installed-world source')
  process.kill(source.pid, 0)
  const bedrock = loadProtocol(), version = '1.26.51', username = 'ArcticRecorder'
  const suffix = String(source.attempt).padStart(3, '0')
  const output = path.join(root, `installed-world-${suffix}.abr`)
  const report = { schema: 1, source, version, username, xboxAuth: false, recordedInstalledWorld: false,
    nativePlaybackVerified: false, deterministicVideoVerified: false, counts: {}, terrainPayloadPackets: 0, negotiation: [] }
  const started = performance.now()
  const ownedEndpoints = () => {
    const result = spawnSync('python', [path.join(__dirname, '../native/lan_probe.py'), String(source.pid)],
      { windowsHide: true, encoding: 'utf8', timeout: 2000, maxBuffer: 65536 })
    if (result.status !== 0) return { error: 'Kernel endpoint query failed' }
    try { return JSON.parse(result.stdout) } catch { return { error: 'Invalid kernel endpoint report' } }
  }
  const nativeNetwork = () => {
    try {
      const file = path.join(root, `arctic-bedrock-${source.pid}.engine.json`)
      if (fs.statSync(file).size > 1048576) return { error: 'Native diagnostics exceed limit' }
      return JSON.parse(fs.readFileSync(file, 'utf8')).probeIsolation?.networkExtensions
    } catch { return { error: 'Native network diagnostics unavailable' } }
  }
  const trace = (event, detail = {}) => {
    if (report.negotiation.length < 64) report.negotiation.push({ event, elapsedMs: Math.round(performance.now() - started), ...detail })
    fs.writeFileSync(path.join(root, 'lan-record-progress.json'), JSON.stringify(report, null, 2))
  }
  const signalSummary = (direction, signal) => {
    if (!['CONNECTREQUEST', 'CONNECTRESPONSE', 'CANDIDATEADD', 'CONNECTERROR'].includes(signal.type)) return
    const detail = { direction, type: signal.type }
    if (signal.type === 'CONNECTREQUEST' || signal.type === 'CONNECTRESPONSE') {
      detail.candidateCount = String(signal.data).split(/\r?\n/).filter(line => line.startsWith('a=candidate:')).length
    }
    if (signal.type === 'CONNECTERROR' && /^\d{1,5}$/.test(signal.data)) detail.code = Number(signal.data)
    // SDP and identity assertions are deliberately excluded from diagnostics.
    if (signal.type === 'CANDIDATEADD') {
      const parts = String(signal.data).split(' ')
      if (/^[\da-fA-F:.]+$/.test(parts[4] || '')) detail.address = parts[4]
      if (/^\d{1,5}$/.test(parts[5] || '')) detail.port = Number(parts[5])
      if (['host', 'srflx', 'relay', 'prflx'].includes(parts[7])) detail.candidateType = parts[7]
    }
    trace('signal', detail)
  }
  let client, recording, problem, start
  try {
    let networkId, nonce
    if (source.transport === 'nethernet') {
      const { NethernetClient } = require('bedrock-protocol/src/nethernet')
      const discovery = new NethernetClient({ host: '127.0.0.1', webrtcBackend: 'werift' })
      const receive = discovery.nethernet.processPacket.bind(discovery.nethernet)
      discovery.nethernet.processPacket = (buffer, remote) => {
        if (remote.address === source.address && remote.port === source.port) receive(buffer, remote)
      }
      try {
        await configureLoopbackDiscovery(discovery.nethernet, source.address)
        const ad = await discovery.ping(5000)
        report.advertisement = { version: ad.version, protocol: ad.protocol, gameVersion: ad.gameVersion,
          levelName: ad.levelName, networkId: String(ad.networkId), acceptsOnlineAuth: ad.acceptsOnlineAuth,
          acceptsSelfSignedAuth: ad.acceptsSelfSignedAuth }
        if (ad.version !== 7 || ad.protocol !== 2193 || ad.acceptsSelfSignedAuth !== true) {
          throw new Error('Installed LAN advertisement does not confirm the supported self-signed protocol')
        }
        if (typeof ad.nonce !== 'string' || !ad.nonce.length || ad.nonce.length > 512) {
          throw new Error('Installed LAN advertisement did not provide its login nonce')
        }
        nonce = ad.nonce
        report.advertisement.nonceLength = nonce.length
        report.advertisement.nonceSha256 = createHash('sha256').update(nonce).digest('hex')
        networkId = ad.networkId
      } finally { discovery.close() }
    }
    recording = new PacketRecording(output, { version, username, uuid: offlineUuid(username),
      producer: 'installed-bedrock-world', sourceSession: source.worldSession }, error => { problem = error })
    client = new bedrock.Client({ host: '127.0.0.1', port: source.port, version, username,
      offline: true, delayedInit: true, autoInitPlayer: true, viewDistance: 2, transport: source.transport,
      skinData: nonce ? { Nonce: nonce } : {},
      nethernet: { networkId, signalling: 'lan', webrtcBackend: 'werift' },
      raknetBackend: 'jsp-raknet', useRaknetWorkers: false, conLog: null, connectTimeout: 15000 })
    client.once('connect_allowed', async () => {
      try {
      if (source.transport === 'nethernet') {
        const peer = client.connection.nethernet, backend = peer.webrtc
        await configureLoopbackDiscovery(peer, source.address)
        client.options.host = source.address
        // This recorder is entirely local. No STUN/TURN services or non-loopback
        // host candidates are needed or admitted into its offer.
        class LoopbackPeer extends backend.RTCPeerConnection {
          constructor(options) {
            super({ ...options, iceServers: [], iceInterfaceAddresses: { udp4: '127.0.0.1', udp6: '::1' },
              iceAdditionalHostAddresses: ['127.0.0.1', '::1'], iceUseIpv4: true, iceUseIpv6: true })
            this.connectionStateChange.subscribe(state => trace('rtc', { state }))
            this.iceConnectionStateChange.subscribe(state => trace('ice', { state }))
          }
          setLocalDescription(description) {
            for (const ice of this.iceTransports) {
              const connection = ice.connection
              if (connection.arcticLoopbackGathering) continue
              connection.arcticLoopbackGathering = true
              // Pinned Werift falls back to every adapter when its interface
              // filter finds no regular (non-loopback) interface match. Apply
              // the restriction before candidate sockets are created instead.
              const gather = connection.getCandidatePromises.bind(connection)
              connection.getCandidatePromises = (addresses, timeout) => gather(addresses.filter(loopbackAddress), timeout)
            }
            return super.setLocalDescription(description)
          }
          setRemoteDescription(description) {
            const candidates = description.sdp.split(/\r?\n/).filter(line => line.startsWith('a=candidate:'))
            trace('remote_description', { candidateCount: candidates.length,
              loopbackCandidates: candidates.filter(line => loopbackCandidate(line.slice(2))).length,
              iceLite: /^a=ice-lite\r?$/m.test(description.sdp) })
            const lines = description.sdp.split(/\r?\n/).filter(line => {
              if (!line.startsWith('a=candidate:') || loopbackCandidate(line.slice(2))) return true
              trace('excluded_non_loopback_candidate'); return false
            })
            return super.setRemoteDescription({ ...description, sdp: lines.join('\r\n') })
          }
          addIceCandidate(candidate) {
            if (candidate?.candidate && !loopbackCandidate(candidate.candidate)) {
              trace('excluded_non_loopback_candidate'); return Promise.resolve()
            }
            return super.addIceCandidate(candidate)
          }
        }
        peer.webrtc = { ...backend, RTCPeerConnection: LoopbackPeer }
        peer.credentials = []
        peer.inactivityTimeoutMs = 15000
        peer.prependListener('disconnect', (_address, reason) => {
          const safeReason = /^(connecterror:\d{1,5}|disconnected)$/.test(reason || '') ? reason : 'unspecified'
          report.transportCloseReason = safeReason
          trace('transport_closed', { reason: safeReason })
          problem ??= new Error(`Installed NetherNet transport closed: ${safeReason}`)
        })
        peer.on('error', error => { problem ??= error; trace('transport_error', { message: String(error.message).slice(0, 300) }) })
        const sendSignal = peer._signalHandler.bind(peer)
        peer._signalHandler = signal => { signalSummary('out', signal); return sendSignal(signal) }
        const receiveSignal = peer.handleSignal.bind(peer)
        peer.handleSignal = signal => {
          if (peer.isExpectedSignal(signal)) signalSummary('in', signal)
          return receiveSignal(signal)
        }
        const receive = peer.processPacket.bind(peer)
        peer.processPacket = (buffer, remote) => {
          if (remote.address === source.address && remote.port === source.port) receive(buffer, remote)
        }
      }
      report.nativeNetworkBefore = nativeNetwork()
      report.ownedEndpointsBefore = ownedEndpoints()
      if (source.transport === 'nethernet') {
        const createChain = client.createClientChain
        let selfSignedToken
        client.createClientChain = function (key, offline) {
          if (offline !== true) throw new Error('Installed recorder permits only a local offline identity')
          createChain.call(this, key, offline)
          if (!selfSignedToken) {
            const claims = JWT.verify(this.multiplayerToken, this.ecdhKeyPair.publicKey,
              { algorithms: ['ES384'], issuer: 'self', audience: 'api://auth-minecraft-services/multiplayer' })
            // An offline player has no Xbox ID. The generic client encodes "0",
            // while native self-signed logins represent absence as an empty ID.
            claims.xid = ''
            selfSignedToken = JWT.sign(claims, this.ecdhKeyPair.privateKey,
              { algorithm: 'ES384', header: { x5u: this.clientX509, typ: undefined } })
          }
          this.multiplayerToken = selfSignedToken
        }
        // NetherNet needs its identity before the SDP offer. The generic
        // protocol client otherwise signs an offline token only in sendLogin,
        // after transport establishment. Prepare that same local identity now.
        client.prependOnceListener('session', () => {
          client.createClientChain(null, true)
          verifyOfflineIdentity(client, username)
          trace('offline_transport_identity', { selfSigned: true, authenticated: false, uuid: client.profile.uuid })
        })
      }
      client.connect()
      } catch (error) { problem ??= error; client.close() }
    })
    client.viewDistance = 2
    const writePacket = client.write
    client.write = function (name, params) {
      if (name === 'login' && source.transport === 'nethernet') {
        const data = JWT.verify(params.tokens.client, this.ecdhKeyPair.publicKey, { algorithms: ['ES384'] })
        const envelope = JSON.parse(params.tokens.identity)
        if (data.Nonce !== nonce || envelope.AuthenticationType !== 2 || envelope.Token !== this.multiplayerToken) {
          throw new Error('Outbound login lost its signed LAN nonce or offline identity')
        }
        report.signedLoginNonceVerified = true
        trace('signed_login_nonce', { length: nonce.length,
          sha256: createHash('sha256').update(nonce).digest('hex') })
      }
      return writePacket.call(this, name, params)
    }
    // The pinned generic client omits max_radius. Keep this private recorder's
    // requested terrain radius explicit in the actual outbound packet.
    const queue = client.queue
    client.queue = function (name, params) {
      if (name === 'request_chunk_radius') params = { ...params, chunk_radius: 2, max_radius: 2 }
      return queue.call(this, name, params)
    }
    client.on('error', error => { problem ??= error })
    client.on('disconnect', packet => {
      report.disconnect = { reason: String(packet.reason).slice(0, 80), message: String(packet.message || '').slice(0, 300) }
      problem = new Error(`Installed server disconnected the recorder: ${report.disconnect.reason}: ${report.disconnect.message}`)
    })
    const packsComplete = () => client.write('resource_pack_client_response', {
      response_status: 'completed', response_status_name: 'resourcepackstackfinished', resourcepackids: [] })
    client.once('resource_packs_info', params => {
      if (params.texture_packs?.length || params.behaviour_packs?.length) {
        problem = new Error('Installed recording source requires external resource packs'); return
      }
      packsComplete(); client.queue('client_cache_status', { enabled: false })
    })
    client.once('resource_pack_stack', packsComplete)
    client.on('network_stack_latency', params => {
      if (params.needs_response) client.queue('network_stack_latency', { timestamp: params.timestamp, needs_response: false })
    })
    client.on('packet', packet => {
      try {
        const { name, params } = packet.data
        if (AUTH.has(name)) { trace('protocol', { packet: name }); return }
        if (name === 'play_status') report.playStatus = params.status
        recording.packet(0, name, packet.fullBuffer)
        report.counts[name] = (report.counts[name] || 0) + 1
        if (name === 'start_game') {
          start = { worldName: params.world_name, dimension: params.dimension, position: params.player_position }
          client.queue('request_chunk_radius', { chunk_radius: 2, max_radius: 2 })
        }
        if (name === 'level_chunk') {
          if (params.highest_subchunk_count != null || params.sub_chunk_count >= 0xfffffffe) {
            if (params.dimension !== 0) throw new Error('Installed recording probe currently requires an overworld source')
            client.queue('subchunk_request', { dimension: params.dimension, origin: { x: params.x, y: 0, z: params.z },
              requests: Array.from({ length: 24 }, (_, i) => ({ x: 0, y: i - 4, z: 0 })) })
          } else if (params.sub_chunk_count > 0 && params.payload?.length) report.terrainPayloadPackets++
        }
        if (name === 'subchunk' && params.entries?.some(entry => entry.result === 'success' && entry.payload?.length)) {
          report.terrainPayloadPackets++
        }
      } catch (error) { problem = error }
    })
    client.init()
    const deadline = performance.now() + 35000
    let observedAt
    while (performance.now() < deadline) {
      if (problem) throw problem
      if (start && report.terrainPayloadPackets > 0) {
        observedAt ??= performance.now()
        if (performance.now() - observedAt >= 5000) break
      }
      if (client._closed) throw new Error('Installed recording connection closed before terrain capture')
      await sleep(50)
    }
    if (!start || report.terrainPayloadPackets < 1) throw new Error('Installed server supplied no world bootstrap and nonempty terrain payload')
    verifyOfflineIdentity(client, username)
    report.identity = { selfSigned: true, authenticated: false, uuid: client.profile.uuid }
    await recording.finish()
    report.recordedInstalledWorld = true; report.startGame = start; report.file = path.basename(output)
    report.bytes = fs.statSync(output).size
  } catch (error) {
    report.error = error.message
    throw error
  } finally {
    report.nativeNetworkAfter = nativeNetwork()
    report.ownedEndpointsAfter = ownedEndpoints()
    client?.close()
    if (recording && !recording.ended) await recording.finish().catch(error => { report.finalizationError = error.message })
    fs.writeFileSync(path.join(root, 'lan-record-evidence.json'), JSON.stringify(report, null, 2))
    fs.writeFileSync(path.join(root, `lan-record-evidence-${suffix}.json`), JSON.stringify(report, null, 2))
    await sleep(150)
  }
}
const watchdog = setTimeout(() => { console.error('Installed world recorder deadline exceeded'); process.exit(1) }, 55000)
record(process.argv[2]).then(() => clearTimeout(watchdog), error => { clearTimeout(watchdog); console.error(error.message); process.exitCode = 1 })
