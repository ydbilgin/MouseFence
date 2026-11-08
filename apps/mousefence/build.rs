fn main() {
    for path in ["res/app.rc", "res/app.manifest", "../../assets/app.ico"] {
        println!("cargo:rerun-if-changed={path}");
    }
    embed_resource::compile("res/app.rc", embed_resource::NONE)
        .manifest_required()
        .expect("MouseFence icon, version and application manifest must be embedded");
}
