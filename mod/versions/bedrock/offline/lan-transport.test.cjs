'use strict'
const test = require('node:test')
const assert = require('node:assert/strict')
const dgram = require('node:dgram')
const { once } = require('node:events')
const { configureLoopbackDiscovery } = require('./lan-transport.cjs')
const close = socket => new Promise(resolve => { try { socket.close(resolve) } catch { resolve() } })

for (const [type, address] of [['udp4', '127.0.0.1'], ['udp6', '::1']]) {
  test(`owned ${type} discovery uses its loopback socket and exact remote port`, { timeout: 5000 }, async () => {
    const server = dgram.createSocket(type), stranger = dgram.createSocket(type), original = dgram.createSocket('udp4')
    let peer
    try {
      server.bind(0, address); await once(server, 'listening')
      original.bind(0, '127.0.0.1'); await once(original, 'listening')
      const port = server.address().port, received = [], errors = []
      let delivered
      const response = new Promise(resolve => { delivered = resolve })
      peer = { socket: original, responses: new Map(), addresses: new Map(),
        reportError(error) { errors.push(error) },
        processPacket(buffer, remote) { received.push([buffer.toString(), remote.address]); delivered() },
        sendDiscoveryRequest() { this.socket.send(Buffer.from('discovery'), port, this.broadcastAddress) } }
      server.on('message', (buffer, remote) => {
        assert.equal(buffer.toString(), 'discovery')
        server.send(Buffer.from('owned-response'), remote.port, remote.address)
      })
      const local = await configureLoopbackDiscovery(peer, address, port)
      await response
      assert.equal(local.address, address)
      assert.deepEqual(received, [['owned-response', address]])
      assert.throws(() => original.address(), { code: 'ERR_SOCKET_DGRAM_NOT_RUNNING' })
      await new Promise((resolve, reject) => stranger.send(Buffer.from('wrong-port'), local.port, address, error => error ? reject(error) : resolve()))
      await new Promise(resolve => setTimeout(resolve, 50))
      assert.equal(received.length, 1)
      assert.deepEqual(errors, [])
    } finally {
      await Promise.all([close(server), close(stranger), close(original), ...(peer ? [close(peer.socket)] : [])])
    }
  })
}
