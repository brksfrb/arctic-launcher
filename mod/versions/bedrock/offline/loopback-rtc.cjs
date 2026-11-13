'use strict'

const { isIP } = require('node:net')

function loopbackAddress(address) {
  return address === '::1' || isIP(address) === 4 && address.startsWith('127.')
    || isIP(address) === 6 && address.toLowerCase().startsWith('::ffff:127.')
}

function loopbackCandidate(candidate) {
  if (typeof candidate !== 'string' || candidate.length > 4096) return false
  const fields = candidate.trim().split(/\s+/)
  return fields.length >= 8 && fields[2].toLowerCase() === 'udp'
    && loopbackAddress(fields[4]) && /^\d{1,5}$/.test(fields[5])
    && Number(fields[5]) > 0 && Number(fields[5]) <= 65535
}

// Scope the policy to the owned peer. Never replace a dependency's global
// constructor or allow its no-interface-match fallback to gather other NICs.
function loopbackBackend(backend, localAddress) {
  if (!['127.0.0.1', '127.0.0.2'].includes(localAddress)) throw new Error('Invalid local RTC interface')
  return { ...backend, RTCPeerConnection: class extends backend.RTCPeerConnection {
    constructor(options) {
      super({ ...options, iceServers: [], iceInterfaceAddresses: { udp4: localAddress },
        iceAdditionalHostAddresses: [localAddress], iceUseIpv4: true, iceUseIpv6: false })
    }
    setLocalDescription(description) {
      for (const ice of this.iceTransports) {
        const connection = ice.connection
        if (connection.arcticLoopbackGathering) continue
        connection.arcticLoopbackGathering = true
        const gather = connection.getCandidatePromises.bind(connection)
        connection.getCandidatePromises = (addresses, timeout) => gather(addresses.filter(address => address === localAddress), timeout)
      }
      return super.setLocalDescription(description)
    }
    setRemoteDescription(description) {
      if (!description || typeof description.sdp !== 'string' || description.sdp.length > 256 * 1024) {
        return Promise.reject(new Error('Invalid local RTC description'))
      }
      const lines = description.sdp.split(/\r?\n/).filter(line =>
        !line.startsWith('a=candidate:') || loopbackCandidate(line.slice(2)))
      return super.setRemoteDescription({ ...description, sdp: lines.join('\r\n') })
    }
    addIceCandidate(candidate) {
      if (candidate?.candidate && !loopbackCandidate(candidate.candidate)) return Promise.resolve()
      return super.addIceCandidate(candidate)
    }
  } }
}

module.exports = { loopbackAddress, loopbackCandidate, loopbackBackend }
