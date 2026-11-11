'use strict'

const crypto = require('node:crypto')
const dgram = require('node:dgram')
const { once } = require('node:events')
const JWT = require('jsonwebtoken')
const { loopbackAddress, loopbackBackend } = require('./loopback-rtc.cjs')

const AUDIENCE = 'api://auth-minecraft-services/multiplayer'
const MAX_TOKEN = 1024 * 1024

// Native self-signed tokens carry cpk in their signed body. Unlike the generic
// protocol client they need not repeat that key in an x5u header. This verifier
// accepts only offline identities and never delegates to OIDC/Xbox services.
function offlineVerifier(nonce) {
  const { parsePublicKey, validateIdentity } = require('bedrock-protocol/src/auth/loginVerifier')
  return async envelope => {
    const token = envelope.multiplayerToken
    if (typeof token !== 'string' || !token.length || token.length > MAX_TOKEN
        || typeof envelope.clientDataToken !== 'string' || envelope.clientDataToken.length > MAX_TOKEN) {
      throw new Error('Local NetherNet login needs bounded self-signed identity and client data')
    }
    const decoded = JWT.decode(token, { complete: true })
    if (!decoded || decoded.header.alg !== 'ES384' || !decoded.payload
        || typeof decoded.payload !== 'object' || typeof decoded.payload.cpk !== 'string') {
      throw new Error('Local NetherNet identity is not a self-signed P-384 token')
    }
    const clientPublicKey = parsePublicKey(decoded.payload.cpk)
    const keyBytes = clientPublicKey.export({ type: 'spki', format: 'der' })
    if (decoded.header.x5u) {
      const headerKey = parsePublicKey(decoded.header.x5u)
      if (!headerKey.export({ type: 'spki', format: 'der' }).equals(keyBytes)) {
        throw new Error('Local NetherNet identity keys disagree')
      }
    }
    const claims = JWT.verify(token, clientPublicKey, { algorithms: ['ES384'], audience: AUDIENCE })
    if (claims.iss != null && claims.iss !== 'self'
        || [claims.xid, claims.XUID, claims.xuid].some(value => value != null && value !== '' && value !== '0' && value !== 0)) {
      throw new Error('Local NetherNet playback requires an offline identity')
    }
    const identity = { displayName: claims.xname, identity: claims.leguuid, XUID: '0' }
    validateIdentity(identity, false)
    const clientData = JWT.verify(envelope.clientDataToken, clientPublicKey, { algorithms: ['ES384'] })
    const received = typeof clientData.Nonce === 'string' ? Buffer.from(clientData.Nonce) : Buffer.alloc(0)
    const expected = Buffer.from(nonce)
    if (received.length !== expected.length || !crypto.timingSafeEqual(received, expected)) {
      throw new Error('Local NetherNet login nonce does not match this playback session')
    }
    return { authentication: { authenticated: false, method: 'offline', issuer: null },
      identity, clientData, clientPublicKey, clientPublicKeyBase64: keyBytes.toString('base64') }
  }
}

function configureReplayNethernet(server, config) {
  if (server.options.offline !== true || server.options.transport !== 'nethernet'
      || !['127.0.0.1', '127.0.0.2'].includes(config.host)) {
    throw new Error('NetherNet playback must use an owned offline loopback server')
  }
  const nonce = crypto.randomBytes(12).toString('base64url')
  server.advertisement.acceptsOnlineAuth = false
  server.advertisement.acceptsSelfSignedAuth = true
  server.advertisement.nonce = nonce
  const verifyLogin = offlineVerifier(nonce)
  const Transport = server.transportServer
  let advertisementTimer
  let stopped = false

  server.transportServer = class extends Transport {
    constructor(options, owner) {
      super({ ...options, credentials: [], iceServers: [], webrtcBackend: 'werift' }, owner)
      const peer = this.nethernet
      peer.credentials = []
      peer.webrtc = loopbackBackend(peer.webrtc, config.host)
      // The pinned listener hardcodes 7551. Keep its parser/signalling methods,
      // but bind this instance to the actual configured local endpoint.
      peer.listen = async () => {
        if (peer._closed) throw new Error('NetherNet playback listener is closed')
        const socket = dgram.createSocket('udp4')
        peer.socket = socket
        socket.on('message', (buffer, remote) => {
          if (!stopped && loopbackAddress(remote.address)) peer.processPacket(buffer, remote)
        })
        socket.on('error', error => peer.emit('error', error))
        try {
          const listening = once(socket, 'listening')
          socket.bind(config.listenPort, config.host)
          await listening
        } catch (error) {
          try { socket.close() } catch { /* failed bind may have no handle */ }
          throw error
        }
      }
    }
  }

  return {
    authenticate(player) {
      const login = player.onLogin
      player.onLogin = function(packet) {
        try {
          const source = packet?.data?.params?.tokens?.identity
          if (typeof source !== 'string' || source.length > MAX_TOKEN
              || JSON.parse(source).AuthenticationType !== 2) {
            throw new Error('Local NetherNet playback accepts only self-signed login envelopes')
          }
        } catch (error) { this.rejectLogin(error); return }
        return login.call(this, packet)
      }
      player.verifyLogin = verifyLogin
    },
    start() {
      if (stopped) return
      const peer = server.transport.nethernet
      const advertise = () => {
        if (stopped || peer._closed) return
        try {
          server.transport.updateAdvertisement()
          // A native local discovery listener uses 7551. Unicast permits the
          // local replay to appear in Worlds without broadcasting to the LAN.
          if (config.host === '127.0.0.2') peer.handleRequest({ address: '127.0.0.1', port: 7551 })
        } catch (error) { server.emit('error', error) }
      }
      advertise()
      advertisementTimer = setInterval(advertise, 1000)
    },
    close() { stopped = true; clearInterval(advertisementTimer) }
  }
}

module.exports = { configureReplayNethernet }
