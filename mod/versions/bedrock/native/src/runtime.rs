use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use hudhook::hooks::{dx11::ImguiDx11Hooks, dx12::ImguiDx12Hooks};
use hudhook::imgui::{self, TextureId};
use hudhook::windows::Win32::Foundation::{HMODULE, HWND, LPARAM, WPARAM};
use hudhook::windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use hudhook::windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, MAPVK_VSC_TO_VK_EX, MapVirtualKeyW,
};
use hudhook::windows::Win32::UI::WindowsAndMessaging::*;
use hudhook::{BeforeWndProc, Hudhook, ImguiRenderLoop, MessageFilter, RenderContext};
use jni::objects::{GlobalRef, JByteArray, JClass, JValue};
use jni::{InitArgsBuilder, JNIEnv, JNIVersion, JavaVM};

use crate::bundle::{Bundle, Renderer};

static STARTED: AtomicBool = AtomicBool::new(false);
static LOG: OnceLock<PathBuf> = OnceLock::new();

fn log(message: impl AsRef<str>) {
    use std::io::Write;
    if let Some(path) = LOG.get()
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{}", message.as_ref());
    }
}

pub fn start(address: usize) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let result = std::panic::catch_unwind(|| boot(address));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(e)) => log(format!("startup failed: {e}")),
        Err(_) => log("startup panicked; native adapter disabled"),
    }
}

fn boot(address: usize) -> Result<(), String> {
    let mut path = [0u16; 32768];
    // SAFETY: DllMain supplies this module's handle, and the writable buffer has
    // the declared capacity. No game memory layout assumptions are made.
    let len = unsafe { GetModuleFileNameW(Some(HMODULE(address as _)), &mut path) } as usize;
    if len == 0 || len >= path.len() {
        return Err("couldn't locate the native module".into());
    }
    let module = PathBuf::from(String::from_utf16_lossy(&path[..len]));
    let dir = module.parent().ok_or("module has no directory")?;
    LOG.set(dir.join(format!("arctic-bedrock-{}.log", std::process::id())))
        .ok();
    log("starting native adapter (engine-dependent features pending)");
    let bundle = Bundle::read(dir)?;
    let isolated_probe = crate::probe_isolation::optional(dir)?;
    crate::native_body::configure_probe(dir, isolated_probe);
    crate::native_clock::configure(dir, isolated_probe);
    crate::native_camera_control::configure(dir, isolated_probe);
    crate::native_flipbook::configure_probe(isolated_probe);
    if isolated_probe {
        log("isolated probe: test profile and network/GDK user guards installed");
    }
    crate::capture::start_control(dir.to_path_buf());
    let jvm = ClientVm::new(&bundle, dir, isolated_probe)?;
    log("shared Java client initialized");
    let render = NativeRender {
        client: jvm,
        input: Mutex::new(Vec::new()),
        menu: AtomicBool::new(false),
        texture: None,
        failure: None,
        frames: 0,
        fps: 0,
        fps_at: Instant::now(),
        ready: dir.join(format!("arctic-bedrock-{}.ready", std::process::id())),
        drew_frame: false,
        engine_frame: None,
        depth_failure: None,
        probe_sequence: None,
        capture_tag: None,
        present_sequence: 0,
        geometry_request: None,
        geometry_stream: crate::native_geometry_stream::Stream::default(),
        geometry_failure: None,
    };
    let hooks = match bundle.renderer {
        Renderer::Dx11 => Hudhook::builder().with::<ImguiDx11Hooks>(render).build(),
        Renderer::Dx12 => Hudhook::builder().with::<ImguiDx12Hooks>(render).build(),
    };
    hooks.apply().map_err(|e| format!("renderer hook: {e:?}"))?;
    if matches!(bundle.renderer, Renderer::Dx12) {
        match crate::depth_tracker::observe_device_creation() {
            Ok(()) => log("DX12 device creation observer installed"),
            Err(error) => log(format!(
                "early DX12 device observation unavailable: {error}"
            )),
        }
    }
    match crate::world_hooks::start(dir.to_path_buf()) {
        Ok(()) => log("engine observer initialization complete (cosmetics bindings pending)"),
        Err(error) => log(format!("engine observers unavailable: {error}")),
    }
    log("renderer hooks installed; awaiting first swap-chain frame");
    Ok(())
}

struct ClientVm {
    vm: Arc<JavaVM>,
    bridge: GlobalRef,
}
struct ClientFrame {
    pixels: Vec<u8>,
    open: bool,
    probe: Option<u64>,
    body_plan: Option<String>,
    attachment_plan: Option<String>,
    capture_tag: Option<crate::capture::FrameTag>,
}

impl ClientVm {
    /// Asset-only request on the serialized client thread. A pinned texture
    /// cannot establish a glint selector, timing/material cohort or world lease.
    fn armor_glint_artwork(
        &mut self,
    ) -> Result<Option<crate::avatar_gpu_upload::GlintArtwork>, String> {
        let mut env = self
            .vm
            .attach_current_thread_as_daemon()
            .map_err(|error| error.to_string())?;
        let class: &JClass = self.bridge.as_obj().into();
        let result = env.with_local_frame(
            3,
            |env| -> jni::errors::Result<Result<Option<Vec<u8>>, &'static str>> {
                let value = env
                    .call_static_method(class, "armorGlintRgba", "()[B", &[])?
                    .l()?;
                if value.is_null() {
                    return Ok(Ok(None));
                }
                let array = JByteArray::from(value);
                if env.get_array_length(&array)? as usize
                    != crate::avatar_gpu_upload::GLINT_RGBA_BYTES
                {
                    return Ok(Err("unexpected glint artwork byte count"));
                }
                Ok(Ok(Some(env.convert_byte_array(array)?)))
            },
        );
        match result {
            Ok(Ok(Some(bytes))) => crate::avatar_gpu_upload::GlintArtwork::from_rgba(bytes)
                .map(Some)
                .map_err(str::to_owned),
            Ok(Ok(None)) => Ok(None),
            Ok(Err(error)) => Err(error.to_owned()),
            Err(error) => Err(java_error(&mut env, error)),
        }
    }
    /// Owned no-readback input, consumed by the same serialized client-thread
    /// owner as frame(). Never invoke mutable Java appearance logic directly
    /// from a native draw worker. The mutable borrow excludes overlapping calls
    /// through this ClientVm while its scoped Java world is temporarily bound.
    fn geometry_input(
        &mut self,
        input: &crate::world_hooks::GeometryInput,
    ) -> Result<Option<crate::avatar_geometry::Frame>, String> {
        if !input.current() {
            return Ok(None);
        }
        let sequence = i64::try_from(input.sequence())
            .map_err(|_| "geometry input sequence exceeds Java range".to_owned())?;
        let mut env = self
            .vm
            .attach_current_thread_as_daemon()
            .map_err(|error| error.to_string())?;
        let class: &JClass = self.bridge.as_obj().into();
        let result = env.with_local_frame(
            6,
            |env| -> jni::errors::Result<Result<Option<Vec<u8>>, &'static str>> {
                let json = env.new_string(input.json())?;
                let session = env.new_string(input.session())?;
                let value = env
                    .call_static_method(
                        class,
                        "worldGeometryFromFrame",
                        "(Ljava/lang/String;Ljava/lang/String;J)[B",
                        &[
                            JValue::Object(&json),
                            JValue::Object(&session),
                            JValue::Long(sequence),
                        ],
                    )?
                    .l()?;
                if value.is_null() {
                    return Ok(Ok(None));
                }
                let array = JByteArray::from(value);
                let length = env.get_array_length(&array)?;
                if length < 0 || length as usize > crate::avatar_geometry::MAX_BYTES {
                    return Ok(Err("Java geometry packet exceeds native copy bound"));
                }
                Ok(Ok(Some(env.convert_byte_array(array)?)))
            },
        );
        let bytes = match result {
            Ok(Ok(Some(bytes))) => bytes,
            Ok(Ok(None)) => return Ok(None),
            Ok(Err(error)) => return Err(error.into()),
            Err(error) => return Err(java_error(&mut env, error)),
        };
        if !input.current() {
            return Ok(None);
        }
        let frame = crate::avatar_geometry::Frame::decode(bytes, input.session(), input.sequence())
            .map_err(str::to_owned)?;
        if !input.matches(&frame) {
            return Err(
                "Java geometry packet disagrees with its native input identity/camera/surface"
                    .into(),
            );
        }
        Ok(Some(frame))
    }
    /// Request copied geometry on the same serialized client thread as frame().
    /// Kept out of the presentation loop until an owning native GPU consumer
    /// exists: staging must not double-render or authorize body suppression.
    #[allow(dead_code)]
    fn geometry(
        &self,
        session: &str,
        sequence: u64,
    ) -> Result<Option<crate::avatar_geometry::Frame>, String> {
        if session.is_empty()
            || session.len() > 64
            || !session
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        {
            return Err("invalid geometry request session".into());
        }
        let sequence_java = i64::try_from(sequence)
            .map_err(|_| "geometry sequence exceeds Java range".to_owned())?;
        let mut env = self
            .vm
            .attach_current_thread_as_daemon()
            .map_err(|e| e.to_string())?;
        let class: &JClass = self.bridge.as_obj().into();
        let result = env.with_local_frame(
            4,
            |env| -> jni::errors::Result<Result<Option<Vec<u8>>, &'static str>> {
                let session_java = env.new_string(session)?;
                let value = env
                    .call_static_method(
                        class,
                        "worldGeometry",
                        "(Ljava/lang/String;J)[B",
                        &[JValue::Object(&session_java), JValue::Long(sequence_java)],
                    )?
                    .l()?;
                if value.is_null() {
                    return Ok(Ok(None));
                }
                let array = JByteArray::from(value);
                let length = env.get_array_length(&array)?;
                if length < 0 || length as usize > crate::avatar_geometry::MAX_BYTES {
                    return Ok(Err("Java geometry packet exceeds native copy bound"));
                }
                Ok(Ok(Some(env.convert_byte_array(array)?)))
            },
        );
        match result {
            Ok(Ok(Some(bytes))) => crate::avatar_geometry::Frame::decode(bytes, session, sequence)
                .map(Some)
                .map_err(str::to_owned),
            Ok(Ok(None)) => Ok(None),
            Ok(Err(error)) => Err(error.to_owned()),
            Err(error) => Err(java_error(&mut env, error)),
        }
    }
    fn appearance_command(
        env: &mut JNIEnv<'_>,
        class: &JClass<'_>,
        request: &crate::native_appearance::Request,
    ) -> Result<serde_json::Value, String> {
        let anchor = if let Some(key) = request.anchor.as_ref() {
            let mut value = crate::native_packet::history_anchor(key, &request.session)?;
            value["relayTimeMs"] = serde_json::json!(
                request
                    .time_ms
                    .ok_or("recording anchor has no relay time")?
            );
            Some(value.to_string())
        } else {
            None
        };
        let result=env.with_local_frame(8,|env| -> jni::errors::Result<String> {
            let session=env.new_string(&request.session)?;
            let owner=match request.owner.as_ref() {Some(owner)=>jni::objects::JObject::from(env.new_string(owner)?),None=>jni::objects::JObject::null()};
            env.call_static_method(class,"checkAppearanceSession","(Ljava/lang/String;)V",&[JValue::Object(&session)])?;
            match request.op.as_str() {
                "history_begin"=> {
                    let path=env.new_string(request.path.as_ref().expect("validated history path").to_string_lossy())?;
                    let time=JValue::Long(i64::from(request.time_ms.expect("validated history origin")));
                    if let Some(anchor)=anchor.as_ref() {
                        let anchor=env.new_string(anchor)?;
                        env.call_static_method(class,"beginAnchoredAppearanceHistory","(Ljava/lang/String;Ljava/lang/String;JLjava/lang/String;Ljava/lang/String;)V",
                            &[JValue::Object(&path),JValue::Object(&session),time,JValue::Object(&owner),JValue::Object(&anchor)])?;
                    } else {
                        env.call_static_method(class,"beginAppearanceHistory","(Ljava/lang/String;Ljava/lang/String;JLjava/lang/String;)V",
                            &[JValue::Object(&path),JValue::Object(&session),time,JValue::Object(&owner)])?;
                    }
                }
                "history_finish"|"history_cancel"=> {
                    env.call_static_method(class,"finishAppearanceHistory","(Ljava/lang/String;Ljava/lang/String;Z)V",
                        &[JValue::Object(&session),JValue::Object(&owner),JValue::Bool(u8::from(request.op=="history_cancel"))])?;
                }
                "save"=> {
                    let path=env.new_string(request.path.as_ref().expect("validated appearance path").to_string_lossy())?;
                    env.call_static_method(class,"saveAppearance","(Ljava/lang/String;)V",&[JValue::Object(&path)])?;
                }
                "load"=> {
                    let path=env.new_string(request.path.as_ref().expect("validated appearance path").to_string_lossy())?;
                    env.call_static_method(class,"loadAppearance","(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",&[JValue::Object(&path),JValue::Object(&session),JValue::Object(&owner)])?;
                }
                "time"=> {env.call_static_method(class,"appearanceTime","(Ljava/lang/String;JLjava/lang/String;)V",&[JValue::Object(&session),JValue::Long(i64::from(request.time_ms.expect("validated appearance time"))),JValue::Object(&owner)])?;}
                "clear"=> {env.call_static_method(class,"clearAppearance","(Ljava/lang/String;Ljava/lang/String;)V",&[JValue::Object(&session),JValue::Object(&owner)])?;}
                _=>{}
            }
            let value=env.call_static_method(class,"appearanceState","()Ljava/lang/String;",&[])?.l()?;
            let value=jni::objects::JString::from(value);let text:String=env.get_string(&value)?.into();Ok(text)
        });
        let text = result.map_err(|error| java_error(env, error))?;
        serde_json::from_str(&text).map_err(|error| error.to_string())
    }
    fn world_gap(&self, present: bool) -> Result<(), String> {
        let mut env = self
            .vm
            .attach_current_thread_as_daemon()
            .map_err(|e| e.to_string())?;
        let class: &JClass = self.bridge.as_obj().into();
        let result = env.call_static_method(class, "worldGap", "(Z)V", &[present.into()]);
        result.map(|_| ()).map_err(|e| java_error(&mut env, e))
    }
    fn engine_texture(&self, texture: &crate::engine::Texture) -> Result<(), String> {
        let mut env = self
            .vm
            .attach_current_thread_as_daemon()
            .map_err(|e| e.to_string())?;
        let class: &JClass = self.bridge.as_obj().into();
        let result = env.with_local_frame(8, |env| -> jni::errors::Result<()> {
            let key = env.new_string(&texture.key)?;
            let bytes = env.byte_array_from_slice(&texture.rgba)?;
            env.call_static_method(
                class,
                "engineTexture",
                "(Ljava/lang/String;II[B)V",
                &[
                    JValue::Object(&key),
                    (texture.width as i32).into(),
                    (texture.height as i32).into(),
                    JValue::Object(&bytes),
                ],
            )?;
            Ok(())
        });
        result.map_err(|e| java_error(&mut env, e))
    }

    fn engine_world(&self, frame: Option<&crate::engine::Frame>) -> Result<(), String> {
        let mut env = self
            .vm
            .attach_current_thread_as_daemon()
            .map_err(|e| e.to_string())?;
        let class: &JClass = self.bridge.as_obj().into();
        let result = env.with_local_frame(8, |env| -> jni::errors::Result<()> {
            if let Some(frame) = frame {
                let json = env.new_string(&frame.json)?;
                let depth = env.new_float_array(frame.depth.len() as i32)?;
                env.set_float_array_region(&depth, 0, &frame.depth)?;
                env.call_static_method(
                    class,
                    "worldFrame",
                    "(Ljava/lang/String;[F)V",
                    &[JValue::Object(&json), JValue::Object(&depth)],
                )?;
            } else {
                let null = jni::objects::JObject::null();
                env.call_static_method(
                    class,
                    "worldFrame",
                    "(Ljava/lang/String;[F)V",
                    &[JValue::Object(&null), JValue::Object(&null)],
                )?;
            }
            Ok(())
        });
        result.map_err(|e| java_error(&mut env, e))
    }

    fn new(bundle: &Bundle, dir: &Path, isolated_probe: bool) -> Result<Self, String> {
        let classpath = format!(
            "-Djava.class.path={}",
            dir.join("arctic-bedrock.jar").display()
        );
        // The disposable test host never contacts the public cosmetics service.
        let test_host = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_owned()))
            .is_some_and(|n| {
                ["render_host.exe", "render_host_dx12.exe"]
                    .iter()
                    .any(|host| n.to_string_lossy().eq_ignore_ascii_case(host))
            });
        let cosmetics = if test_host {
            "-Darctic.cosmetics.url=http://127.0.0.1:9"
        } else {
            "-Darctic.bedrock.native=true"
        };
        let probe_option =
            isolated_probe.then(|| format!("-Darctic.bedrock.probe.directory={}", dir.display()));
        let mut args = InitArgsBuilder::new()
            .version(JNIVersion::V8)
            .option(&classpath)
            .option("-Djava.awt.headless=true")
            .option(cosmetics)
            .option("-Xms32m")
            // Full-resolution world/depth surfaces need a larger bounded
            // ceiling; retain the small initial heap.
            .option("-Xmx512m");
        if let Some(option) = &probe_option {
            // The isolated asset service binds IPv4 loopback. Its strict bind
            // guard maps wildcard IPv6 to ::1, which cannot connect to mapped
            // IPv4 peers; use the matching stack only in this disposable probe.
            args = args
                .option(option)
                .option("-Djava.net.preferIPv4Stack=true");
        }
        let args = args.build().map_err(|e| e.to_string())?;
        let vm = JavaVM::with_libjvm(args, || Ok(bundle.jvm())).map_err(|e| e.to_string())?;
        let bridge = {
            let mut env = vm
                .attach_current_thread_as_daemon()
                .map_err(|e| e.to_string())?;
            let result = env.with_local_frame(16, |env| -> jni::errors::Result<GlobalRef> {
                let class = env.find_class("com/arcticlauncher/bedrock/NativeBridge")?;
                let config = env.new_string(bundle.config_dir.to_string_lossy())?;
                let version = env.new_string(&bundle.game_version)?;
                let name = env.new_string(&bundle.offline_name)?;
                let reference = env.new_string(
                    bundle
                        .java_reference_jar
                        .as_ref()
                        .map(|path| path.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                )?;
                env.call_static_method(
                    &class,
                    "init",
                    "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
                    &[
                        JValue::Object(&config),
                        JValue::Object(&version),
                        JValue::Object(&name),
                        JValue::Object(&reference),
                    ],
                )?;
                env.new_global_ref(class)
            });
            result.map_err(|e| java_error(&mut env, e))?
        };
        let vm = Arc::new(vm);
        crate::native_emote_pose::bind(Arc::clone(&vm), bridge.clone())?;
        Ok(Self { vm, bridge })
    }

    fn frame(
        &self,
        size: [i32; 2],
        fps: i32,
        mouse: [i32; 2],
        keys: &[u8; 256],
        events: &[i32],
    ) -> Result<ClientFrame, String> {
        let mut env = self
            .vm
            .attach_current_thread_as_daemon()
            .map_err(|e| e.to_string())?;
        let class: &JClass = self.bridge.as_obj().into();
        let result = env.with_local_frame(16, |env| -> jni::errors::Result<ClientFrame> {
            crate::native_appearance::consume(|request| {
                Self::appearance_command(env, class, request)
            });
            let (metadata, clean) = crate::capture::frame_options();
            env.call_static_method(
                class,
                "captureView",
                "(ZZ)V",
                &[
                    JValue::Bool(u8::from(metadata)),
                    JValue::Bool(u8::from(clean)),
                ],
            )?;
            let key_array = env.new_boolean_array(256)?;
            env.set_boolean_array_region(&key_array, 0, keys)?;
            let event_array = env.new_int_array(events.len() as i32)?;
            env.set_int_array_region(&event_array, 0, events)?;
            let pixels = env
                .call_static_method(
                    class,
                    "frame",
                    "(IIIII[Z[I)[B",
                    &[
                        size[0].into(),
                        size[1].into(),
                        fps.into(),
                        mouse[0].into(),
                        mouse[1].into(),
                        JValue::Object(&key_array),
                        JValue::Object(&event_array),
                    ],
                )?
                .l()?;
            let pixels = env.convert_byte_array(JByteArray::from(pixels))?;
            let open = env.call_static_method(class, "menuOpen", "()Z", &[])?.z()?;
            let probe = env
                .call_static_method(class, "frameProbeSequence", "()J", &[])?
                .j()?;
            let capture_tag = if metadata {
                let value = env
                    .call_static_method(class, "frameCaptureMetadata", "()Ljava/lang/String;", &[])?
                    .l()?;
                if value.is_null() {
                    None
                } else {
                    let value = jni::objects::JString::from(value);
                    let text: String = env.get_string(&value)?.into();
                    crate::capture::FrameTag::parse(&text)
                }
            } else {
                None
            };
            let requested = env
                .call_static_method(class, "nativeCosmeticsRequested", "()Z", &[])?
                .z()?;
            crate::native_body::set_user_enabled(requested);
            env.call_static_method(
                class,
                "nativeCosmeticsMode",
                "(Z)V",
                &[JValue::Bool(u8::from(crate::native_body::enabled()))],
            )?;
            // Always refresh the plan, including mode-off cleanup of immutable
            // emote programs and pins for already acknowledged native draws.
            let body_plan = {
                let value = env
                    .call_static_method(class, "bodyPlan", "()Ljava/lang/String;", &[])?
                    .l()?;
                let value = jni::objects::JString::from(value);
                Some(env.get_string(&value)?.into())
            };
            let attachment_plan = {
                let value = env
                    .call_static_method(class, "attachmentPlan", "()Ljava/lang/String;", &[])?
                    .l()?;
                let value = jni::objects::JString::from(value);
                Some(env.get_string(&value)?.into())
            };
            Ok(ClientFrame {
                pixels,
                open,
                probe: (probe > 0 && probe <= 1_000_000_000).then_some(probe as u64),
                body_plan,
                attachment_plan,
                capture_tag,
            })
        });
        if result.is_err() {
            crate::native_body::set_user_enabled(false);
            crate::native_body::update_ready(None);
            crate::native_attachment_replay::invalidate();
        }
        result.map_err(|e| java_error(&mut env, e))
    }
}

fn java_error(env: &mut jni::JNIEnv<'_>, error: jni::errors::Error) -> String {
    let detail = (|| -> jni::errors::Result<String> {
        let exception = env.exception_occurred()?;
        env.exception_clear()?;
        if exception.is_null() {
            return Ok(String::new());
        }
        let message = env
            .call_method(exception, "toString", "()Ljava/lang/String;", &[])?
            .l()?;
        let message = jni::objects::JString::from(message);
        Ok(env.get_string(&message)?.into())
    })()
    .unwrap_or_default();
    // Never leave an exception pending across a native render callback.
    let _ = env.exception_clear();
    format!("{error}: {detail}")
}

struct GeometryRequest {
    input: crate::world_hooks::GeometryInput,
    device: hudhook::windows::Win32::Graphics::Direct3D12::ID3D12Device,
    queue: hudhook::windows::Win32::Graphics::Direct3D12::ID3D12CommandQueue,
}
struct NativeRender {
    client: ClientVm,
    input: Mutex<Vec<i32>>,
    menu: AtomicBool,
    texture: Option<TextureId>,
    failure: Option<String>,
    frames: u32,
    fps: i32,
    fps_at: Instant,
    ready: PathBuf,
    drew_frame: bool,
    engine_frame: Option<(String, u64)>,
    depth_failure: Option<String>,
    probe_sequence: Option<u64>,
    capture_tag: Option<crate::capture::FrameTag>,
    present_sequence: u64,
    geometry_request: Option<GeometryRequest>,
    geometry_stream: crate::native_geometry_stream::Stream,
    geometry_failure: Option<String>,
}

impl NativeRender {
    /// Run only after the ordinary client frame has imported textures and
    /// updated appearance/controllers. Geometry emission itself does not tick
    /// those controllers again, and never runs on a backend draw worker.
    fn stage_world_geometry(&mut self) -> Result<(), String> {
        let Some(request) = self.geometry_request.take() else {
            return Ok(());
        };
        if !self.geometry_stream.available() {
            return Ok(());
        }
        let Some(frame) = self.client.geometry_input(&request.input)? else {
            return Ok(());
        };
        let active_glint = frame.material.is_some()
            && frame.batches.iter().any(|batch| {
                batch.model_submission.is_some_and(|model| {
                    matches!(
                        model.glint,
                        Some(crate::avatar_geometry::GlintSelection::Active(_))
                    )
                })
            });
        let artwork = if active_glint {
            Some(
                self.client
                    .armor_glint_artwork()?
                    .ok_or("selected glint has no pinned artwork")?,
            )
        } else {
            None
        };
        self.geometry_stream.stage(
            &request.device,
            &request.queue,
            request.input,
            frame,
            artwork,
        )
    }
    fn geometry_result(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.geometry_stream.reject(&error);
            if self.geometry_failure.as_ref() != Some(&error) {
                log(format!("native world geometry unavailable: {error}"));
                self.geometry_failure = Some(error);
            }
        } else {
            self.geometry_failure = None;
        }
    }
}

impl ImguiRenderLoop for NativeRender {
    fn on_dx12_before_frame(
        &mut self,
        swap: &hudhook::windows::Win32::Graphics::Dxgi::IDXGISwapChain3,
        queue: &hudhook::windows::Win32::Graphics::Direct3D12::ID3D12CommandQueue,
    ) {
        use hudhook::windows::Win32::Graphics::Direct3D12::ID3D12Device;
        crate::avatar_gpu_upload::collect_retired();
        crate::avatar_draw_submit::collect_retired();
        crate::native_draw_assets::collect();
        crate::native_committed_resources::collect();
        crate::native_descriptor_resources::collect();
        self.geometry_request = None;
        let geometry_result = self.geometry_stream.poll();
        self.geometry_result(geometry_result);
        crate::world_hooks::begin_gpu_frame();
        // SAFETY: hudhook supplies a live swap chain from this presenting thread.
        let result = (|| -> Result<(), String> {
            let device: ID3D12Device = unsafe { swap.GetDevice() }.map_err(|e| e.to_string())?;
            crate::depth_tracker::initialize(&device)?;
            let desc = unsafe { swap.GetDesc() }.map_err(|e| e.to_string())?;
            if self.geometry_stream.available() {
                match crate::depth_tracker::geometry_world(
                    queue,
                    desc.BufferDesc.Width,
                    desc.BufferDesc.Height,
                ) {
                    Ok(input) => {
                        self.geometry_request = input.map(|input| GeometryRequest {
                            input,
                            device: device.clone(),
                            queue: queue.clone(),
                        })
                    }
                    Err(error) => self.geometry_result(Err(error)),
                }
            }
            if let Some((scope, depth)) = crate::depth_tracker::capture_world(
                queue,
                desc.BufferDesc.Width,
                desc.BufferDesc.Height,
            )? {
                crate::world_hooks::accept_depth(scope, depth);
            }
            Ok(())
        })();
        if let Err(error) = result {
            if self.depth_failure.as_ref() != Some(&error) {
                log(format!("world depth unavailable: {error}"));
                self.depth_failure = Some(error);
            }
        } else {
            self.depth_failure = None;
        }
    }
    fn on_dx11_frame(&mut self, swap: &hudhook::windows::Win32::Graphics::Dxgi::IDXGISwapChain) {
        self.present_sequence = self.present_sequence.saturating_add(1);
        let marker = self.probe_sequence.take();
        let mut tag = self.capture_tag.take();
        if let Some(tag) = &mut tag {
            tag.gpu = Some(crate::capture::PresentInfo {
                api: "dx11",
                sequence: self.present_sequence,
                buffer_index: None,
                buffer_count: unsafe { swap.GetDesc() }.ok().map(|d| d.BufferCount),
            });
        }
        if crate::capture::needs_frame(tag.as_ref())
            || crate::probe_isolation::needs_tagged_frame(marker)
        {
            match crate::readback::dx11(swap) {
                Ok((width, height, rgba)) => {
                    crate::probe_isolation::frame(width, height, &rgba, marker);
                    crate::capture::frame(width, height, rgba, tag);
                }
                Err(error) => crate::capture::fail(error),
            }
        }
    }
    fn on_dx12_frame(
        &mut self,
        swap: &hudhook::windows::Win32::Graphics::Dxgi::IDXGISwapChain3,
        queue: &hudhook::windows::Win32::Graphics::Direct3D12::ID3D12CommandQueue,
    ) {
        self.present_sequence = self.present_sequence.saturating_add(1);
        let marker = self.probe_sequence.take();
        let mut tag = self.capture_tag.take();
        if let Some(tag) = &mut tag {
            tag.gpu = Some(crate::capture::PresentInfo {
                api: "dx12",
                sequence: self.present_sequence,
                buffer_index: Some(unsafe { swap.GetCurrentBackBufferIndex() }),
                buffer_count: unsafe { swap.GetDesc() }.ok().map(|d| d.BufferCount),
            });
        }
        let wanted = crate::capture::needs_frame(tag.as_ref())
            || crate::probe_isolation::needs_tagged_frame(marker);
        if crate::native_clock::probe_enabled() {
            use hudhook::windows::Win32::Graphics::Direct3D12::ID3D12Resource;
            use hudhook::windows::core::Interface;
            if let Ok(buffer) =
                unsafe { swap.GetBuffer::<ID3D12Resource>(swap.GetCurrentBackBufferIndex()) }
            {
                let presented =
                    crate::native_scene_target::presented(&buffer, queue.as_raw() as usize, wanted);
                if let Some(tag) = &mut tag {
                    tag.presented_constants = presented;
                }
            }
        }
        if wanted {
            match crate::readback::dx12(swap, queue) {
                Ok((width, height, rgba)) => {
                    crate::probe_isolation::frame(width, height, &rgba, marker);
                    crate::capture::frame(width, height, rgba, tag);
                }
                Err(error) => crate::capture::fail(error),
            }
        }
        crate::depth_tracker::end_frame();
    }
    fn initialize<'a>(&'a mut self, ctx: &mut imgui::Context, _: &'a mut dyn RenderContext) {
        // Texture IDs belong to one renderer instance, not the Java client.
        // Swap-chain recreation builds a new renderer and invalidates old IDs.
        self.texture = None;
        self.probe_sequence = None;
        self.capture_tag = None;
        self.geometry_request = None;
        self.geometry_stream.reset();
        self.geometry_failure = None;
        crate::native_body::update_ready(None);
        crate::native_attachment_replay::invalidate();
        self.failure = None;
        ctx.set_ini_filename(None);
        log("swap-chain renderer initialized");
    }

    fn before_render<'a>(
        &'a mut self,
        ctx: &mut imgui::Context,
        renderer: &'a mut dyn RenderContext,
    ) {
        self.capture_tag = None;
        self.frames += 1;
        let elapsed = self.fps_at.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.fps = (self.frames as f64 / elapsed.as_secs_f64()).round() as i32;
            self.frames = 0;
            self.fps_at = Instant::now();
        }
        if self.failure.is_some() {
            return;
        }
        let display = ctx.io().display_size;
        if display.iter().any(|v| !v.is_finite() || *v < 2.0) {
            return;
        }
        // The world layer follows each presentation, at its pixel resolution.
        // Reusing a 33 ms old overlay while native models keep moving makes
        // replacement bodies/capes lag independently of the game's frame rate.
        // Fit oversized surfaces uniformly instead of stretching each axis.
        // Java retains its separate two-pixel GUI scale for HUD/menu layout.
        let scale = (3840.0 / display[0]).min(2160.0 / display[1]).min(1.0);
        let size = [
            (display[0] * scale).ceil().clamp(1.0, 3840.0) as i32,
            (display[1] * scale).ceil().clamp(1.0, 2160.0) as i32,
        ];
        crate::engine::surface(size[0], size[1]);
        crate::world_hooks::publish_world(size[0] as u32, size[1] as u32);
        for texture in crate::engine::textures() {
            if let Err(e) = self.client.engine_texture(&texture) {
                log(format!("engine texture rejected: {e}"));
            }
        }
        let world = crate::engine::latest();
        let sequence = world.as_ref().map(|f| (f.session.clone(), f.sequence));
        if world.is_none() {
            if let Err(error) = self.client.world_gap(crate::world_hooks::world_present()) {
                log(format!("world gap rejected: {error}"));
            }
            self.engine_frame = None;
        } else if sequence != self.engine_frame {
            if let Err(e) = self.client.engine_world(world.as_deref()) {
                log(format!("engine frame rejected: {e}"));
                let _ = self.client.engine_world(None);
            } else {
                crate::world_hooks::delivered(world.as_ref().map(|f| f.sequence));
            }
            self.engine_frame = sequence;
        }
        let mouse = [
            (ctx.io().mouse_pos[0] / display[0] * size[0] as f32) as i32,
            (ctx.io().mouse_pos[1] / display[1] * size[1] as f32) as i32,
        ];
        let mut keys = [0u8; 256];
        // SAFETY: Win32 key polling reads physical state only; it does not
        // dereference pointers into the game or change its controls.
        for (vk, key) in keys.iter_mut().enumerate() {
            *key = u8::from(unsafe { GetAsyncKeyState(vk as i32) } < 0);
        }
        let events = self
            .input
            .lock()
            .map(|mut q| std::mem::take(&mut *q))
            .unwrap_or_default();
        let result = self
            .client
            .frame(size, self.fps, mouse, &keys, &events)
            .and_then(|frame| {
                let ClientFrame {
                    pixels,
                    open,
                    probe,
                    body_plan,
                    attachment_plan,
                    capture_tag,
                } = frame;
                if pixels.len() != size[0] as usize * size[1] as usize * 4 {
                    return Err("shared client returned an invalid pixel buffer".into());
                }
                match self.texture {
                    Some(id) => {
                        renderer.replace_texture(id, &pixels, size[0] as u32, size[1] as u32)
                    }
                    None => renderer
                        .load_texture(&pixels, size[0] as u32, size[1] as u32)
                        .map(|id| self.texture = Some(id)),
                }
                .map_err(|e| e.to_string())?;
                // Publish the marker only after these exact pixels were uploaded.
                // Consume it at the post-render readback, never on a later frame.
                self.probe_sequence = probe;
                self.capture_tag = capture_tag;
                crate::native_body::update_ready(body_plan.as_deref());
                crate::native_attachment_replay::update(attachment_plan.as_deref());
                self.menu.store(open, Ordering::Release);
                ctx.io_mut().mouse_draw_cursor = open && !crate::capture::frame_options().1;
                Ok(())
            });
        if let Err(e) = result {
            self.geometry_request = None;
            self.geometry_stream.reset();
            crate::native_body::update_ready(None);
            crate::native_attachment_replay::invalidate();
            log(format!("render failed: {e}"));
            self.failure = Some(e);
            self.menu.store(false, Ordering::Release);
        } else {
            let geometry_result = self.stage_world_geometry();
            self.geometry_result(geometry_result);
        }
    }

    fn render(&mut self, ui: &mut imgui::Ui) {
        if let Some(error) = &self.failure {
            ui.window("Arctic Bedrock startup error").build(|| {
                ui.text_wrapped(error);
            });
        } else if let Some(texture) = self.texture {
            ui.get_background_draw_list()
                .add_image(texture, [0.0, 0.0], ui.io().display_size)
                .build();
            if !self.drew_frame {
                self.drew_frame = true;
                log("first shared-client frame rendered; native preview ready");
                let _ = std::fs::write(&self.ready, "native-preview\n");
            }
        }
    }

    fn before_wnd_proc(&self, _: HWND, message: u32, w: WPARAM, l: LPARAM) -> BeforeWndProc {
        let event = match message {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let mut vk = w.0 as i32;
                if vk == 0x10 {
                    // WM_KEYDOWN reports VK_SHIFT; resolve the right-hand scan code.
                    vk = unsafe { MapVirtualKeyW(((l.0 >> 16) & 0xFF) as u32, MAPVK_VSC_TO_VK_EX) }
                        as i32;
                } else if vk == 0x11 {
                    vk = if l.0 & (1 << 24) != 0 { 0xA3 } else { 0xA2 };
                } else if vk == 0x12 {
                    vk = if l.0 & (1 << 24) != 0 { 0xA5 } else { 0xA4 };
                }
                // A held Right Shift must not repeatedly close/reopen the page.
                if vk == 0xA1 && (l.0 & (1 << 30)) != 0 {
                    None
                } else {
                    Some([0, vk])
                }
            }
            WM_CHAR => Some([1, w.0 as i32]),
            WM_LBUTTONDOWN => Some([2, 0]),
            WM_RBUTTONDOWN => Some([2, 1]),
            WM_MBUTTONDOWN => Some([2, 2]),
            WM_XBUTTONDOWN => Some([2, if (w.0 >> 16) & 0xFFFF == 1 { 3 } else { 4 }]),
            WM_LBUTTONUP => Some([3, 0]),
            WM_RBUTTONUP => Some([3, 1]),
            WM_MBUTTONUP => Some([3, 2]),
            WM_XBUTTONUP => Some([3, if (w.0 >> 16) & 0xFFFF == 1 { 3 } else { 4 }]),
            WM_MOUSEWHEEL => Some([4, ((w.0 >> 16) as u16 as i16) as i32]),
            WM_MOUSEMOVE if w.0 & 1 != 0 => Some([5, 0]),
            _ => None,
        };
        if let Some(event) = event
            && let Ok(mut q) = self.input.lock()
            && q.len() < 4096
        {
            q.extend_from_slice(&event);
        }
        BeforeWndProc::Continue
    }

    fn message_filter(&self, _: &imgui::Io) -> MessageFilter {
        if self.menu.load(Ordering::Acquire) {
            MessageFilter::InputAll
        } else {
            MessageFilter::empty()
        }
    }
}
