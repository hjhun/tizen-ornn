fn main() {
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
