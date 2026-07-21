use std::env;
use std::fs;
use std::path::PathBuf;

fn parse_component(name: &str) -> u16 {
    env::var(name)
        .unwrap_or_else(|_| panic!("missing Cargo package version component {name}"))
        .parse::<u16>()
        .unwrap_or_else(|_| panic!("invalid Cargo package version component {name}"))
}

fn parse_preview() -> (u16, u16) {
    let prerelease =
        env::var("CARGO_PKG_VERSION_PRE").expect("missing Cargo package prerelease component");
    if prerelease.is_empty() {
        return (0, 0);
    }

    let mut components = prerelease.split('.');
    assert_eq!(components.next(), Some("preview"), "unsupported prerelease");
    let preview = components
        .next()
        .expect("missing preview number")
        .parse::<u16>()
        .expect("invalid preview number");
    let revision = components
        .next()
        .expect("missing preview revision")
        .parse::<u16>()
        .expect("invalid preview revision");
    assert!(
        components.next().is_none(),
        "unsupported Cargo package prerelease component {prerelease}"
    );
    (preview, revision)
}

fn main() {
    let major = parse_component("CARGO_PKG_VERSION_MAJOR");
    let minor = parse_component("CARGO_PKG_VERSION_MINOR");
    let patch = parse_component("CARGO_PKG_VERSION_PATCH");
    let (preview, revision) = parse_preview();
    let generated = format!(
        "pub const SDK_MAJOR: u16 = {major};\n\
         pub const SDK_MINOR: u16 = {minor};\n\
         pub const SDK_PATCH: u16 = {patch};\n\
         pub const SDK_PREVIEW: u16 = {preview};\n\
         pub const SDK_REVISION: u16 = {revision};\n"
    );
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("missing Cargo OUT_DIR"));
    fs::write(out_dir.join("sdk_version.rs"), generated)
        .expect("failed to write generated SDK version constants");
}
