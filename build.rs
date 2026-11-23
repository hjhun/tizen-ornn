fn main() {
    println!("cargo:rerun-if-changed=config/version");
    let version = include_str!("config/version").trim_end();
    assert_eq!(
        version,
        env!("CARGO_PKG_VERSION"),
        "config/version must match Cargo.toml"
    );
    assert!(
        version.split('.').count() == 3
            && version
                .split('.')
                .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())),
        "expected MAJOR.MINOR.RELEASE"
    );
    println!("cargo:rerun-if-changed=packaging/ornn.rc");
    println!("cargo:rerun-if-changed=packaging/art/tizen-ornn.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for(
            "packaging/ornn.rc",
            ["tizen-ornn-gui"],
            embed_resource::NONE,
        )
        .manifest_required()
        .expect("compile the Tizen Ornn application icon");
    }
}
