use std::{env, fs::File, io::Write, path::PathBuf, process::Command};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tinytemplate::TinyTemplate;

#[derive(Deserialize, Serialize, Clone)]
struct BundleConfiguration {
    name: String,
    short_description: String,
    rom: String,
    netplay_rom: Option<String>,
    version: Option<String>,
    cf_bundle_identifier: String,
    wix_upgrade_code: String,
    manufacturer: String,
}

fn main() -> Result<()> {
    let stretch_path =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("src/audio/stretch");
    let signalsmith_path = stretch_path.join("signalsmith-stretch");

    if !signalsmith_path.join("signalsmith-stretch.h").exists() {
        Command::new("git")
            .args(["submodule", "update", "--init"])
            .current_dir(signalsmith_path.clone())
            .status()
            .expect("Git is needed to retrieve the signalsmith-stretch source files");
    }

    println!("cargo:rerun-if-changed=config/linux/*");
    println!("cargo:rerun-if-changed=config/macos/*");
    println!("cargo:rerun-if-changed=config/windows/*");
    println!("cargo:rerun-if-changed=config/windows/wix/*");

    println!("cargo:rerun-if-changed=src/audio/stretch/signalsmith-stretch/**");
    println!("cargo:rerun-if-changed=src/audio/stretch/signalsmith-stretch-wrapper.*");
    println!("cargo:rerun-if-changed=src/audio/stretch/mod.rs");
    let mut code = cxx_build::bridge(stretch_path.join("mod.rs"));
    let code = code
        .file(stretch_path.join("signalsmith-stretch-wrapper.cpp"))
        .flag_if_supported("-std=c++11");

    #[cfg(not(target_os = "windows"))]
    code.flag("-O3");
    #[cfg(target_os = "windows")]
    code.flag("/O2");

    code.compile("signalsmith-stretch");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("config/windows/icon_256x256.ico");
        res.compile().expect("Could not attach exe icon");
    }

    let mut bundle_config: BundleConfiguration =
        serde_yaml::from_str(include_str!("config/config.yaml"))?;

    let mut tt = TinyTemplate::new();

    tt.add_template(
        "main.wxs",
        include_str!("config/windows/wix/main.wxs-template"),
    )?;
    tt.add_template(
        "bundle.desktop",
        include_str!("config/linux/bundle.desktop-template"),
    )?;
    tt.add_template(
        "Info.plist",
        include_str!("config/macos/Info.plist-template"),
    )?;

    println!("cargo:rustc-env=NB_WINDOW_TITLE={}", bundle_config.name);
    println!("cargo:rustc-env=NB_ROM=../{}", bundle_config.rom);

    println!(
        "cargo:rustc-env=NB_NETPLAY_ROM=../{}",
        bundle_config
            .clone()
            .netplay_rom
            .unwrap_or(bundle_config.rom.clone())
    );

    bundle_config
        .version
        .get_or_insert(env!("CARGO_PKG_VERSION").to_string());

    File::create("config/windows/wix/main.wxs")?
        .write_all(tt.render("main.wxs", &bundle_config)?.as_bytes())?;

    File::create("config/linux/bundle.desktop")?
        .write_all(tt.render("bundle.desktop", &bundle_config)?.as_bytes())?;

    File::create(std::path::Path::new("config/macos/Info.plist"))?
        .write_all(tt.render("Info.plist", &bundle_config)?.as_bytes())?;
    Ok(())
}
