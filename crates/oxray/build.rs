//! Marks release builds with `oxray_release_profile`.
//!
//! A release profile can still turn debug assertions on. The cfg follows the
//! profile name so that combination does not enable capture.

#![forbid(unsafe_code)]

fn main() {
    println!("cargo::rustc-check-cfg=cfg(oxray_release_profile)");
    println!("cargo:rerun-if-env-changed=PROFILE");
    let Ok(profile) = std::env::var("PROFILE") else {
        return;
    };
    if profile == "release" {
        println!("cargo:rustc-cfg=oxray_release_profile");
    }
}
