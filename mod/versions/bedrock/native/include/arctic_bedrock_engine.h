#ifndef ARCTIC_BEDROCK_ENGINE_H
#define ARCTIC_BEDROCK_ENGINE_H
#include <stddef.h>
#include <stdint.h>

/* Resolve these exports in arctic_bedrock.dll with GetProcAddress.
 * This API is an input boundary; it does not install Bedrock engine hooks.
 * All calls copy caller-owned inputs before returning and may run on an engine
 * thread. JNI/model rendering runs on Arctic's render thread.
 *
 * Return codes: 0 = queued, -1 = invalid input, -2 = old/duplicate sequence,
 * -3 = texture queue full, -4 = internal failure. Queuing is not a promise
 * that malformed schema fields or missing assets will render: Java validates
 * the complete snapshot before publishing it. See ENGINE.md.
 * Pointers must remain readable for their declared sizes during the call.
 * Depth must be float-aligned. Passing invalid pointers is undefined behavior.
 */
#ifdef __cplusplus
extern "C" {
#endif
/* Width in upper 32 bits, height in lower 32; zero before first render.
 * This is the world-overlay pixel surface, fitted uniformly to 3840x2160.
 * HUD/menu logical dimensions use a separate GUI scale. */
uint64_t arctic_bedrock_surface_v1(void);
/* UTF-8 JSON (not NUL terminated), max 1 MiB; DirectX normalized depth [0,1],
 * row-major, top-left origin, exactly snapshot width*height float elements;
 * snapshot dimensions are 1..3840 by 1..2160.
 * Optional cameraRelative declares the model/projection coordinate space:
 * true uses camera-relative positions, false absolute positions. Omission
 * retains legacy projected-depth translucent sorting.
 * Monotonically increasing sequence per session, max 128 actors. Snapshots
 * expire after 500 ms without a fresh submission. Clear on disconnect too. */
int32_t arctic_bedrock_world_v1(const uint8_t *json, size_t len,
                              const float *depth, size_t pixels);
/* engine:<name>, ASCII letters/digits/_/- only after prefix, max 120 chars.
 * Unpremultiplied RGBA8, top-left origin, 1..1024 on each axis. Upload when
 * an asset changes, not every frame. Pending uploads: 128 assets / 64 MiB. */
int32_t arctic_bedrock_texture_v1(const uint8_t *key, size_t key_len,
                                uint32_t width, uint32_t height,
                                const uint8_t *rgba, size_t bytes);
void arctic_bedrock_clear_world_v1(void);
/* Capture the actual swap-chain image after overlay rendering. Bounded UTF-8
 * JSON, max 16 KiB, {"op":"start","ffmpeg":"ABSOLUTE_PATH","output":"NEW.mp4",
 * "fps":30,"frames":300} or {"op":"stop"}. MP4/H.264, no audio.
 * Scoped {"op":"reserve","lease_ms":120000} owns the capture slot without
 * opening an encoder; renew uses the same lease field (5000..120000 ms).
 * "activate" accepts indexed start fields and requires the same unexpired
 * captureId reservation. Stop releases a reservation without creating files.
 * Optional start fields: mode="indexed", session="native_PID_EPOCH",
 * clean_view=true. Indexed mode waits for explicit {"op":"sample","index":0,
 * "min_sequence":1,"ticks":1,"partial":0.25,"clock_ms":62,
 * "camera":[0,64,0,0,0,70]} matching a newly uploaded native world layer.
 * Optional sample frames=1..120 requests that many fresh matching world layers
 * at the same pose/time; each accepted layer still gets its own next index.
 * Receipts and *.frames.jsonl identify accepted samples and raw GPU hashes.
 * Native receipts also record GPU callback sequence and DX12 buffer index.
 * Indexed settle_frames defaults to 3 (1..8): identical fresh images must span
 * every DX12 buffer before encoding. Set 1 only for raw capture diagnostics.
 * clean_view hides Arctic UI/cursor; native HUD control is separate.
 * A receipt acknowledges owned/enqueued pixels; only complete confirms encoding.
 * 0 accepted, -1 invalid buffer, -2 rejected control, -3 internal failure. */
int32_t arctic_bedrock_capture_v1(const uint8_t *json, size_t len);
/* Frames submitted in upper 32 bits, state in lower 32:
 * 0 idle, 1 waiting, 2 capturing, 3 finishing, 4 complete, 5 error,
 * 6 reserved (no encoder), 7 reservation cancelled.
 * Detailed status: arctic-bedrock-PID.capture.json beside the DLL. */
uint64_t arctic_bedrock_capture_status_v1(void);
/* Serialized appearance archive control, max 16 KiB schema-1 UTF-8 JSON.
 * Fields: session, increasing sequence, requestId, op=save/load/time/clear/state;
 * save/load use an absolute path ending .arctic-looks.zip; time uses timeMs.
 * load/time/clear accept an owner token (1-64 alphanumeric/_/-). An owned
 * mount requires that same token for replacement, timing and clearing.
 * history_begin requires a new path, timeMs origin and owner; history_finish
 * and history_cancel require that owner. Finish queues a writer: poll state
 * until history.stage=complete or failed before treating the file as saved.
 * history_begin may carry anchor={recordingId,sequence,requestId,marker,
 * generation,replaySequence}. timeMs is then the relay's observed queue time.
 * The consumer requires that exact completed packet receipt, under three
 * seconds old, and derives the first state time from its native tick clock.
 * This is initial marker correlation, not packet/terrain application proof.
 * Admission: 0 accepted, -1 invalid buffer, -2 rejected. Queued requests expire
 * after 2 seconds. Once executing, file IO can outlive a host wait; do not infer
 * cancellation from timeout. Completion: arctic-bedrock-PID.appearance.json.
 * Java load preparation has a 20-second pre-commit deadline. A successful load
 * reserves retained texture space for a later clear; session end releases it.
 * Completion describes Java state, not native suppression or presented pixels. */
int32_t arctic_bedrock_appearance_v1(const uint8_t *json, size_t len);
/* DX12 callbacks, excluding TEST presents. Diagnostic only; this is not a
 * deterministic simulation or replay-frame acknowledgement. */
uint64_t arctic_bedrock_renderer_frames_v1(void);
#ifdef __cplusplus
}
#endif
#endif
