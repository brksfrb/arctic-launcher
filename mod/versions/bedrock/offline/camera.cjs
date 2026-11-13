'use strict'

// Camera edits use the public Bedrock camera protocol, not private engine offsets.
// Runtime preset IDs are indices in the complete CameraPresets packet.
const MAX_TIME = 86_400_000
const { createHash } = require('node:crypto')
const PRESET = 'arctic:replay_editor'
const CAMERA_PACKETS = new Set(['camera_instruction', 'camera_spline'])
const PATH_YAW = new WeakMap()

// Match core/replay/CameraPath.java, including its float keyframe yaw and
// nearest-turn tie rule. Keep this separate from the saved poses: older linear
// paths still retain their original endpoint angles.
function unwrappedYaw(frames) {
  const yaw = [Math.fround(frames[0].pose.yaw)]
  for (let i = 1; i < frames.length; i++) {
    let delta = Math.fround(Math.fround(frames[i].pose.yaw) - yaw[i - 1])
    delta = Math.fround(delta - Math.fround(360 * Math.floor(Math.fround(delta / 360) + 0.5)))
    yaw.push(Math.fround(yaw[i - 1] + delta))
  }
  return yaw
}

// Uniform Catmull-Rom, with duplicated endpoint controls as on Java.
function spline(p0, p1, p2, p3, u) {
  const u2 = u * u, u3 = u2 * u
  return 0.5 * (2 * p1 + (-p0 + p2) * u + (2 * p0 - 5 * p1 + 4 * p2 - p3) * u2
    + (-p0 + 3 * p1 - 3 * p2 + p3) * u3)
}

function yawTrack(path) {
  let yaw = PATH_YAW.get(path)
  if (!yaw) { yaw = unwrappedYaw(path.keyframes); PATH_YAW.set(path, yaw) }
  return yaw
}

function endpoint(path, index) {
  const frames = path.keyframes, p = frames[index].pose
  if (frames[index].ease !== 'catmull_rom' && (index === 0 || frames[index - 1].ease !== 'catmull_rom')) return p
  return { ...p, yaw: yawTrack(path)[index], pitch: Math.fround(p.pitch), fov: Math.fround(p.fov) }
}

function pose(value) {
  return checkedPose(value, false)
}

// Input keys keep their existing yaw limit. Java's evaluated nearest-turn
// track may accumulate more turns, without changing any geometric bounds.
function checkedPose(value, unwrappedYaw) {
  if (!value || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).some(k => !['x', 'y', 'z', 'yaw', 'pitch', 'fov'].includes(k))) throw new Error('Invalid camera pose')
  const result = { x: value.x, y: value.y, z: value.z, yaw: value.yaw, pitch: value.pitch, fov: value.fov ?? 70 }
  if (!Object.values(result).every(Number.isFinite) || ['x', 'y', 'z'].some(k => Math.abs(result[k]) > 30_000_000)
    || (!unwrappedYaw && Math.abs(result.yaw) > 360_000) || Math.abs(result.pitch) > 90 || result.fov < 1 || result.fov > 179) throw new Error('Camera coordinates, angles or FOV are out of range')
  return result
}

function cameraPath(value, duration = MAX_TIME) {
  if (!value || value.format !== 1 || Object.keys(value).some(k => !['format', 'keyframes'].includes(k))
    || !Array.isArray(value.keyframes) || !value.keyframes.length || value.keyframes.length > 4096) throw new Error('Camera path needs format 1 and 1–4096 keyframes')
  let previous = -1
  const path = { format: 1, keyframes: value.keyframes.map(frame => {
    if (!frame || Object.keys(frame).some(k => !['timeMs', 'pose', 'ease'].includes(k))
      || !Number.isSafeInteger(frame.timeMs) || frame.timeMs <= previous || frame.timeMs > duration) throw new Error('Camera keyframes must have increasing times within the replay')
    if (frame.ease !== undefined && !['linear', 'smooth', 'hold', 'catmull_rom'].includes(frame.ease)) throw new Error('Unknown camera interpolation')
    previous = frame.timeMs
    return { timeMs: frame.timeMs, pose: pose(frame.pose), ease: frame.ease ?? 'linear' }
  }) }
  PATH_YAW.set(path, unwrappedYaw(path.keyframes))
  return path
}

// Same binary intent layout as offline::CameraPath::sha256 in Rust. Hash the
// accepted path, preserving float64 bits and avoiding JSON property ordering.
function pathSha256(path) {
  const checked = cameraPath(path), hash = createHash('sha256')
  hash.update(Buffer.from('arctic-bedrock-camera-path-v1\0'))
  const header = Buffer.alloc(8); header.writeUInt32LE(checked.format, 0); header.writeUInt32LE(checked.keyframes.length, 4); hash.update(header)
  const ease = ['linear', 'smooth', 'hold', 'catmull_rom']
  for (const frame of checked.keyframes) {
    const bytes = Buffer.alloc(53); bytes.writeUInt32LE(frame.timeMs, 0); bytes[4] = ease.indexOf(frame.ease)
    for (const [index, key] of ['x', 'y', 'z', 'yaw', 'pitch', 'fov'].entries()) bytes.writeDoubleLE(frame.pose[key], 5 + index * 8)
    hash.update(bytes)
  }
  return hash.digest('hex')
}

function sample(path, time) {
  if (!Number.isFinite(time) || time < 0 || time > MAX_TIME) throw new Error('Invalid camera sample time')
  const frames = path.keyframes
  if (time <= frames[0].timeMs) return endpoint(path, 0)
  let low = 0, high = frames.length - 1
  while (low < high) { const mid = Math.ceil((low + high) / 2); if (frames[mid].timeMs <= time) low = mid; else high = mid - 1 }
  const a = frames[low], b = frames[low + 1]
  if (!b) return endpoint(path, low)
  let t = (time - a.timeMs) / (b.timeMs - a.timeMs)
  if (a.ease === 'catmull_rom') {
    const p0 = frames[Math.max(0, low - 1)].pose, p3 = frames[Math.min(frames.length - 1, low + 2)].pose
    const yaw = yawTrack(path)
    const out = {}
    for (const key of ['x', 'y', 'z']) out[key] = spline(p0[key], a.pose[key], b.pose[key], p3[key], t)
    out.pitch = Math.max(-90, Math.min(90, spline(Math.fround(p0.pitch), Math.fround(a.pose.pitch), Math.fround(b.pose.pitch), Math.fround(p3.pitch), t)))
    out.yaw = spline(yaw[Math.max(0, low - 1)], yaw[low], yaw[low + 1], yaw[Math.min(frames.length - 1, low + 2)], t)
    out.fov = Math.fround(a.pose.fov) + Math.fround(Math.fround(b.pose.fov) - Math.fround(a.pose.fov)) * t
    // Curves can overshoot the keyframe positions. Reject an out-of-world
    // sample before it can be serialized, instead of silently moving the path.
    return checkedPose(out, true)
  }
  if (a.ease === 'smooth') t = t * t * (3 - 2 * t)
  if (a.ease === 'hold') t = 0
  const out = {}
  for (const key of ['x', 'y', 'z', 'pitch', 'fov']) out[key] = a.pose[key] + (b.pose[key] - a.pose[key]) * t
  const delta = ((b.pose.yaw - a.pose.yaw) % 360 + 540) % 360 - 180
  out.yaw = a.pose.yaw + delta * t
  return out
}

// Keep the same floor/remainder conversion as CameraCurve.sample_at. Dividing
// the full frame offset before adding a large origin has a different floating
// operation order, even though both expressions describe the same timestamp.
function sampleTime(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).some(key => !['timeMs', 'remainder', 'denominator'].includes(key))
    || !Number.isSafeInteger(value.timeMs) || value.timeMs < 0 || value.timeMs > MAX_TIME
    || !Number.isSafeInteger(value.denominator) || value.denominator < 1 || value.denominator > 1000
    || !Number.isSafeInteger(value.remainder) || value.remainder < 0 || value.remainder >= value.denominator
    || value.timeMs === MAX_TIME && value.remainder !== 0) throw new Error('Invalid rational camera sample time')
  return value.timeMs + value.remainder / value.denominator
}

function frameTime(origin, index, fps) {
  if (!Number.isSafeInteger(origin) || origin < 0 || origin > MAX_TIME
    || !Number.isSafeInteger(index) || index < 0
    || !Number.isSafeInteger(fps) || fps < 1 || fps > 120) throw new Error('Invalid camera frame time')
  const numerator = index * 1000
  if (!Number.isSafeInteger(numerator)) throw new Error('Camera frame index exceeds its exact range')
  const time = { timeMs: origin + Math.floor(numerator / fps), remainder: numerator % fps, denominator: fps }
  sampleTime(time)
  return time
}

function sampleAt(path, time) { return sample(path, sampleTime(time)) }

// ReplayViewer casts CameraPath's double yaw/pitch/FOV to its float camera
// interface before the game applies them. Round the unwrapped Java yaw here,
// before wirePose normalizes equivalent turns. Raw samples stay available for
// editing; fixed cameras and older non-Java interpolation retain their rules.
function sampleForView(path, time) {
  const result = sample(path, time), frames = path.keyframes
  let low = 0, high = frames.length - 1
  while (low < high) { const mid = Math.ceil((low + high) / 2); if (frames[mid].timeMs <= time) low = mid; else high = mid - 1 }
  if (frames[low].ease !== 'catmull_rom') return result
  return { ...result, yaw: Math.fround(result.yaw), pitch: Math.fround(result.pitch), fov: Math.fround(result.fov) }
}
function sampleForViewAt(path, time) { return sampleForView(path, sampleTime(time)) }

function presets(original = []) {
  if (!Array.isArray(original) || original.length > 4096) throw new Error('Invalid recorded camera presets')
  const list = original.filter(p => p.name !== PRESET)
  // Keep all recorded IDs stable. Replace an existing Arctic entry in place.
  const existing = original.findIndex(p => p.name === PRESET)
  const entry = { name: PRESET, parent: 'minecraft:free', position: {}, rotation: {},
    audio_listener: 0, player_effects: false, apply_inherited_starting_rotation: false }
  if (existing >= 0) { const result = original.slice(); result[existing] = entry; return { presets: result, id: existing } }
  return { presets: [...list, entry], id: list.length }
}

function wirePose(value) {
  const checked = checkedPose(value, true)
  // Equivalent turns share one float32 wire angle; retaining very large
  // unwrapped keyframe yaw here needlessly loses native rotation precision.
  const yaw = ((checked.yaw + 180) % 360 + 360) % 360 - 180
  const floatYaw = Math.fround(yaw)
  return { x: Math.fround(checked.x), y: Math.fround(checked.y), z: Math.fround(checked.z),
    yaw: floatYaw === 180 ? -180 : floatYaw, pitch: Math.fround(checked.pitch), fov: Math.fround(checked.fov) }
}
function instruction(value, id) {
  const target = wirePose(value)
  return { instruction_set: { runtime_id: id, position: { x: target.x, y: target.y, z: target.z },
    rotation: { x: target.pitch, z: target.yaw }, remove_ignore_starting_values: false },
  fov: { field_of_view: target.fov, ease_time: 0, ease_type: 'linear', clear: false } }
}

// minecraft-data 3.117.0 incorrectly uses the SetInstruction u8 easing enum
// for FovInstruction. Mojang's 2193 schema uses an easing_function STRING here.
// Patch only the pinned, verified protocol before serializers are compiled.
function patchCameraSchema() {
  const types = require('minecraft-data')('bedrock_1.26.51').protocol.types
  const fov = types.packet_camera_instruction[1].find(field => field.name === 'fov')
  const ease = fov.type[1][1].find(field => field.name === 'ease_type')
  ease.type = 'string'
}

module.exports = { pose, cameraPath, pathSha256, sample, sampleTime, frameTime, sampleAt, sampleForView, sampleForViewAt, presets, wirePose, instruction, patchCameraSchema, CAMERA_PACKETS }
