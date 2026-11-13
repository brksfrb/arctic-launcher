'use strict'

// Manual integration check of the built Rust CLI, first-run npm setup, process
// readiness, unsigned upstream login and helper shutdown. No game account used.
const assert = require('node:assert/strict')
const { spawn } = require('node:child_process')
const dgram = require('node:dgram')
const { once } = require('node:events')
const { resolve } = require('node:path')
const { mkdtempSync, writeFileSync, unlinkSync, rmdirSync } = require('node:fs')
const { tmpdir } = require('node:os')
const { deflateSync, crc32 } = require('node:zlib')
const { loadProtocol, offlineUuid } = require('./connector.cjs')

function fixturePng(width, height, color) {
  const chunk = (name, bytes) => {
    const tag = Buffer.from(name), size = Buffer.alloc(4), crc = Buffer.alloc(4)
    size.writeUInt32BE(bytes.length)
    crc.writeUInt32BE(crc32(Buffer.concat([tag, bytes])))
    return Buffer.concat([size, tag, bytes, crc])
  }
  const header = Buffer.alloc(13)
  header.writeUInt32BE(width, 0); header.writeUInt32BE(height, 4)
  header[8] = 8; header[9] = 6
  const rows = Buffer.concat(Array.from({ length: height }, () =>
    Buffer.concat([Buffer.from([0]), Buffer.alloc(width * 4, Buffer.from(color))])))
  return Buffer.concat([Buffer.from('89504e470d0a1a0a', 'hex'), chunk('IHDR', header),
    chunk('IDAT', deflateSync(rows)), chunk('IEND', Buffer.alloc(0))])
}

async function freePort() {
  const socket = dgram.createSocket('udp4')
  socket.bind(0, '127.0.0.1')
  await once(socket, 'listening')
  const { port } = socket.address()
  await new Promise(done => socket.close(done))
  return port
}

async function main() {
  const exe = resolve(process.argv[2])
  const data = resolve(process.argv[3])
  const assets = mkdtempSync(resolve(tmpdir(), 'arctic-bedrock-offline-'))
  const skinFile = resolve(assets, 'skin.png'), capeFile = resolve(assets, 'cape.png')
  writeFileSync(skinFile, fixturePng(64, 64, [13, 27, 191, 255]))
  writeFileSync(capeFile, fixturePng(128, 64, [71, 53, 29, 255]))
  const bedrock = loadProtocol()
  const { CURRENT_VERSION } = require('bedrock-protocol/src/options')
  const port = await freePort()
  let local = await freePort()
  while (local === port) local = await freePort()
  const server = new bedrock.Server({ host: '127.0.0.1', port, version: CURRENT_VERSION,
    offline: true, raknetBackend: 'jsp-raknet' })
  let cli, client
  try {
    await server.listen()
    let resolveJoin
    const joined = new Promise(resolve => { resolveJoin = resolve })
    server.on('connect', player => player.on('join', () => resolveJoin(player)))
    cli = spawn(exe, ['--data-dir', data, '--json', 'bedrock', 'offline', `127.0.0.1:${port}`,
      '--name', 'LauncherGuest', '--listen-port', String(local), '--skin', skinFile, '--cape', capeFile, '--slim'],
    { stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true })
    let output = '', diagnostics = '', errors = '', resolveReady, resolveConnected, rejectReady
    const ready = new Promise((resolve, reject) => { resolveReady = resolve; rejectReady = reject })
    const connected = new Promise(resolve => { resolveConnected = resolve })
    cli.stderr.on('data', data => { errors += data.toString() })
    cli.on('error', rejectReady)
    cli.on('exit', code => rejectReady(new Error(`CLI exited ${code}: ${errors}\n${diagnostics.slice(-4000)}`)))
    cli.stdout.on('data', data => {
      diagnostics += data.toString()
      output += data.toString()
      let end
      while ((end = output.indexOf('\n')) >= 0) {
        const line = output.slice(0, end); output = output.slice(end + 1)
        let event
        try { event = JSON.parse(line) } catch { continue }
        if (event.event === 'offline_ready') resolveReady(event)
        if (event.event === 'joined') resolveConnected(event)
      }
    })
    const timeout = setTimeout(() => rejectReady(new Error(`CLI startup timed out: ${errors}`)), 110_000)
    let event
    try { event = await ready } finally { clearTimeout(timeout) }
    assert.equal(event.xbox_auth, false)
    assert.equal(event.username, 'LauncherGuest')
    assert.equal(event.address, `127.0.0.1:${local}`)
    client = bedrock.createClient({ host: '127.0.0.1', port: local, version: CURRENT_VERSION,
      username: 'LocalPlayer', offline: true, skipPing: true, raknetBackend: 'jsp-raknet',
      useRaknetWorkers: false, conLog: null })
    client.on('error', error => console.error(error.message))
    const player = await joined
    assert.equal(player.profile.name, 'LauncherGuest')
    assert.equal(player.profile.uuid, offlineUuid('LauncherGuest'))
    assert.equal(player.authentication.authenticated, false)
    assert.equal(player.authentication.method, 'offline')
    assert.equal(player.skinData.SkinData, Buffer.alloc(64 * 64 * 4, Buffer.from([13, 27, 191, 255])).toString('base64'))
    assert.equal(player.skinData.CapeData, Buffer.alloc(64 * 32 * 4, Buffer.from([71, 53, 29, 255])).toString('base64'))
    assert.equal(player.skinData.ArmSize, 'slim')
    assert.equal((await connected).xbox_auth, false)
    console.log('Rust CLI/bootstrap/unsigned identity/custom skin/HD cape conversion smoke passed')
  } finally {
    client?.close()
    await new Promise(resolve => setTimeout(resolve, 150))
    unlinkSync(skinFile); unlinkSync(capeFile); rmdirSync(assets)
    if (cli && cli.exitCode === null) { const exited = once(cli, 'exit'); cli.kill(); await exited }
    await new Promise(resolve => setTimeout(resolve, 400))
    for (const player of Object.values(server.clients)) player.close('test finished')
    await server.close()
    await new Promise(resolve => setTimeout(resolve, 150))
  }
}

const watchdog = setTimeout(() => { console.error('Offline launcher smoke timed out'); process.exit(1) }, 120_000)
main().then(() => clearTimeout(watchdog), error => { console.error(error); process.exitCode = 1; clearTimeout(watchdog) })
