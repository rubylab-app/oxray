//! `OXRAY=off` disables capture without a builder flag.

use std::process::Command;

#[test]
fn environment_off_disables_the_handle() {
    if std::env::var_os("OXRAY_OFF_CHILD").is_some() {
        let oxray = oxray::Oxray::builder().build().unwrap();
        assert_eq!(oxray.mode(), oxray::Mode::Disabled);
        return;
    }
    let status = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("environment_off_disables_the_handle")
        .env("OXRAY", "off")
        .env("OXRAY_OFF_CHILD", "1")
        .status()
        .unwrap();
    assert!(status.success());
}
