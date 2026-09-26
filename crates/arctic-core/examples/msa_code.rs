//! Start a Microsoft device-code sign-in and wait for it:
//! `ARCTIC_MSA_CLIENT_ID=<id> cargo run -p arctic-core --example msa_code [-- --wait]`
use std::sync::atomic::AtomicBool;

use arctic_core::auth::microsoft::{self, MsaConfig};
use arctic_core::storage::DataDirs;

fn main() {
    let dirs = DataDirs::new(std::env::temp_dir().join("arctic-msa-check"));
    let cfg = MsaConfig::load(&dirs).expect("client id");
    println!("live endpoints: {}", cfg.is_live());
    let code = microsoft::start_device_code(&cfg).expect("device code");
    println!(
        "go to {}?otc={} (code {}), expires in {}s",
        code.verification_uri, code.user_code, code.user_code, code.expires_in
    );
    if std::env::args().any(|a| a == "--wait") {
        match microsoft::finish_device_code(&cfg, &code, &AtomicBool::new(false)) {
            Ok(account) => println!("signed in as {} ({})", account.username, account.uuid),
            Err(e) => println!("sign-in failed: {e}"),
        }
    }
}
