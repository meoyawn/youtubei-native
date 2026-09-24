use std::path::{Path, PathBuf};
use std::process::Command;

fn run(command: &mut Command) {
    let output = command.output().expect("start native build command");
    assert!(
        output.status.success(),
        "{command:?}\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let build = std::env::var_os("HERMES_BUILD_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(".hermes/build"))
        .canonicalize()
        .expect("Build the pinned Hermes checkout first; see README.md");
    let compiler = build.join("bin/shermes");
    for input in [
        "js/youtube.js",
        "js/rust-apis.js",
        "js/host.js",
        "js/driver.js",
        "package.json",
        "nub.lock",
    ] {
        println!("cargo:rerun-if-changed={input}");
    }
    println!("cargo:rerun-if-changed={}", compiler.display());
    for name in ["MACOSX_DEPLOYMENT_TARGET", "HERMES_BUILD_DIR"] {
        println!("cargo:rerun-if-env-changed={name}");
    }

    // The upstream bundle already preserves its public names. This only joins
    // our adapter to that file; it does not traverse the package module graph.
    run(Command::new("nub")
        .current_dir(&root)
        .args([
            "--no-env-file",
            "exec",
            "--no-check",
            "esbuild",
            "js/youtube.js",
            "--bundle",
            "--format=iife",
        ])
        .arg(format!("--outfile={}", out.join("youtube.js").display())));

    let mut objects = Vec::new();
    for (name, input, typed) in [
        ("host", root.join("js/host.js"), true),
        ("youtube", out.join("youtube.js"), false),
        ("driver", root.join("js/driver.js"), true),
    ] {
        let object = out.join(format!("{name}.o"));
        let mut hermes = Command::new(&compiler);
        hermes
            .args([
                "-c",
                "-O",
                "-Wc,=-O1",
                "-g1",
                "-Xes6-block-scoping",
                "-Xasync-generators",
            ])
            .arg(format!("-exported-unit=youtubei_{name}"));
        if typed {
            hermes.arg("-typed");
        }
        run(hermes.arg("-o").arg(&object).arg(input));
        objects.push(object);
    }
    let archive = out.join("libyoutubei.a");
    // Recreate the archive so removed native units cannot survive a rebuild.
    if archive.exists() {
        std::fs::remove_file(&archive).unwrap();
    }
    run(Command::new("ar").arg("rcs").arg(&archive).args(objects));

    link(&out, "youtubei");
    link(&build.join("lib"), "hermesvmlean_a");
    link(&build.join("jsi"), "jsi");
    link(
        &build.join("external/boost/boost_1_86_0/libs/context"),
        "boost_context",
    );
    println!("cargo:rustc-link-lib=c++");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}

fn link(directory: &Path, library: &str) {
    println!("cargo:rustc-link-search=native={}", directory.display());
    println!("cargo:rustc-link-lib=static={library}");
}
