'use strict'
// Per-instance transport setup for the pinned NetherNet implementation. Its
// constructor opens udp4 on all interfaces; replace only that owned discovery
// socket after its bind callback has completed, before starting negotiation.
const dgram = require('node:dgram')
const { once } = require('node:events')

async function configureLoopbackDiscovery(peer, address, port = 7551) {
  if (!['127.0.0.1', '::1'].includes(address) || !Number.isInteger(port) || port < 1 || port > 65535
      || peer._closed || peer._connecting || peer.connection) {
    throw new Error('Loopback discovery requires a fresh owned NetherNet peer')
  }
  const original = peer.socket
  try { original.address() } catch { await once(original, 'listening') }
  const socket = dgram.createSocket({ type: address === '::1' ? 'udp6' : 'udp4', ipv6Only: address === '::1' })
  const reportError = error => peer.reportError(error)
  socket.on('error', reportError)
  socket.on('message', (buffer, remote) => {
    if (remote.address === address && remote.port === port) peer.processPacket(buffer, remote)
  })
  try {
    const bound = once(socket, 'listening')
    socket.bind(0, address)
    await bound
  } catch (error) {
    try { socket.close() } catch { /* bind may have failed before a handle existed */ }
    throw error
  }
  peer.socket = socket
  peer.broadcastAddress = address
  peer.responses.clear()
  peer.addresses.clear()
  await new Promise(resolve => original.close(resolve))
  peer.sendDiscoveryRequest()
  return socket.address()
}

module.exports = { configureLoopbackDiscovery }
