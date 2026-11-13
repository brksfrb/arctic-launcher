//! Fast headless check of the production Java HTTP path under the same network
//! hooks as the installed-game probe. Uses only owned sentinel directories.
#[cfg(windows)]
fn main() {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 5 {
        eprintln!("java_lookup_host OUTPUT_DIR JAVA_HOME CLIENT_JAR GDK_DLL");
        std::process::exit(2);
    }
    let root = std::path::PathBuf::from(&args[1]);
    let result = run(&root, &args[2], &args[3], &args[4]);
    let passed = result.is_ok();
    let report = serde_json::json!({"passed":passed,"result":result.unwrap_or_else(|e| e),
        "isolation":arctic_bedrock::probe_isolation::status()});
    std::fs::write(root.join("java-lookup.json"), report.to_string()).unwrap();
    println!("{report}");
    std::process::exit(if passed { 0 } else { 1 });
}

#[cfg(windows)]
fn run(
    root: &std::path::Path,
    java: &std::ffi::OsStr,
    jar: &std::ffi::OsStr,
    gdk: &std::ffi::OsStr,
) -> Result<String, String> {
    use hudhook::windows::{
        Win32::System::LibraryLoader::LoadLibraryW,
        core::{HSTRING, PCWSTR},
    };
    use jni::{InitArgsBuilder, JNIVersion, JavaVM, objects::JValue};
    use std::io::{Read, Write};
    let work = || -> Result<String, Box<dyn std::error::Error>> {
        let name = HSTRING::from(gdk);
        unsafe {
            LoadLibraryW(PCWSTR(name.as_ptr()))?;
        }
        std::fs::create_dir(root.join("source"))?;
        std::fs::create_dir(root.join("profile"))?;
        arctic_bedrock::probe_isolation::install_loopback(
            &root.join("profile"),
            &root.join("source"),
        )?;
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let address = listener.local_addr()?;
        let request_file = root.join("http-request.txt");
        std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while std::time::Instant::now() < deadline {
                if let Ok((mut socket, _)) = listener.accept() {
                    let _ = socket.set_nonblocking(false);
                    let _ = socket.set_read_timeout(Some(std::time::Duration::from_secs(10)));
                    let mut request = Vec::new();
                    while request.len() < 4096 && !request.windows(4).any(|v| v == b"\r\n\r\n") {
                        let mut block = [0u8; 512];
                        match socket.read(&mut block) {
                            Ok(0) | Err(_) => break,
                            Ok(count) => request.extend_from_slice(&block[..count]),
                        }
                    }
                    let _ = std::fs::write(&request_file, &request);
                    let _=socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 11\r\nConnection: close\r\n\r\nlocal-looks");
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        let classpath = format!("-Djava.class.path={}", std::path::Path::new(jar).display());
        let options = InitArgsBuilder::new()
            .version(JNIVersion::V8)
            .option(&classpath)
            .option("-Djava.awt.headless=true")
            .option("-Djava.net.preferIPv4Stack=true")
            .build()?;
        let vm = JavaVM::with_libjvm(options, || {
            Ok(std::path::Path::new(java).join("bin/server/jvm.dll"))
        })?;
        let mut env = vm.attach_current_thread()?;
        let url = env.new_string(format!("http://{address}/v1/players?uuids=test"))?;
        let response = env.call_static_method(
            "com/arcticlauncher/client/looks/Http",
            "getText",
            "(Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;",
            &[
                JValue::Object(&url),
                JValue::Object(&jni::objects::JObject::null()),
            ],
        );
        let response = match response {
            Ok(value) => value,
            Err(error) => {
                let exception = env.exception_occurred()?;
                env.exception_clear()?;
                let text = env
                    .call_method(exception, "toString", "()Ljava/lang/String;", &[])?
                    .l()?;
                let detail: String = env.get_string(&jni::objects::JString::from(text))?.into();
                return Err(format!("Java lookup: {error}: {detail}").into());
            }
        };
        let text = jni::objects::JString::from(response.l()?);
        let text: String = env.get_string(&text)?.into();
        if text != "local-looks" {
            return Err("HTTP body changed".into());
        }
        Ok(text)
    };
    work().map_err(|e| e.to_string())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
}
