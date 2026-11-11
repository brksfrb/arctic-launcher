# Arctic modifications

Vendored from hudhook 0.9.3, published by Andrea Venuta under the retained MIT
license. MinHook's bundled license is retained under vendor/minhook.

Arctic adds default no-op `ImguiRenderLoop::on_dx11_frame` and `on_dx12_frame`
callbacks after overlay submission and before Present, with the swap chain and
(for DX12) its validated direct command queue. Pipeline dispatches these callbacks.
Arctic also adds `on_dx12_before_frame` and a queue observer that can serialize
depth-state metadata around the original ExecuteCommandLists call. The original
submission is forwarded exactly once, including if the observer panics. The
depth observer uses public D3D12 APIs and preserves game commands and arguments.
The DX12 backend uploads replacement textures on the validated rendering queue,
transitions existing shader resources back to COPY_DEST, and releases both copy
location COM references after the upload fence completes. This prevents races
with outstanding overlay draws and a per-frame upload allocation leak.

Swap-chain replacement releases the active-context mutex before resetting the
pipeline. Arctic's render loop clears texture IDs when a new renderer initializes.
Size-only DX12 resizes also rebuild that pipeline so the Java texture dimensions
continue to match even when the format and buffer count stay unchanged.
DXGI_PRESENT_TEST calls forward directly without drawing or consuming a capture
frame. Bounded DX12 diagnostics expose frame, error, reset and device-removal
information for the isolated installed-game probe. Diagnostics also retain the
actual non-TEST Present HRESULT, including occlusion status. An optional
process-lifetime result-filter callback supports an isolated offscreen test;
it runs after the real call, preserves the actual result in diagnostics, and
falls back to that result if the callback panics. No filter is installed for
ordinary bundles or default probes. The hidden DX12 regression
host exercises resize, format/buffer-count changes, swap-chain replacement and
both Present test paths, with decoded MP4 color checks in each capture phase.

There are no changes to signature scanning or game authentication.
