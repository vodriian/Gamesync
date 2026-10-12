fn main() {
    println!("cargo:rerun-if-changed=icon/app-icon.ico");
    println!("cargo:rerun-if-changed=icon/windows.rc");
    #[cfg(windows)]
    embed_resource::compile_for(
        "icon/windows.rc",
        ["gamesync-desktop"],
        embed_resource::NONE,
    )
    .manifest_required()
    .expect("Could not compile the Windows application icon");
}
