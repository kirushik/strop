//! Build-time facts: the commit hash About's colophon prints, and (on
//! Windows targets) the executable's embedded icon resource.

fn main() {
    // Short hash for "commit <hash>" in About; absent git (a tarball
    // build) falls back to option_env!'s None → "unknown". Re-run when
    // HEAD moves so the hash can't go stale in incremental builds.
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned());
    if let Some(hash) = hash {
        println!("cargo:rustc-env=STROP_GIT_HASH={hash}");
    }
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    // The exe's own icon (packaging/windows/strop.rc → the xtask-generated
    // .ico). Target-gated, not host-gated; and the .ico is a CI product
    // (release.yml runs `xtask icons` first), so a local build without
    // packaging/generated/ simply ships iconless rather than failing.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::path::Path::new("../../packaging/generated/strop.ico").exists()
    {
        embed_resource::compile("../../packaging/windows/strop.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("embedding the Windows icon resource failed");
    }
}
