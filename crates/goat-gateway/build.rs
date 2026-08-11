use std::{path::Path, process::Command};

fn main() {
    let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web");
    println!("cargo::rerun-if-changed={}/src", web.display());
    println!("cargo::rerun-if-changed={}/index.html", web.display());
    println!("cargo::rerun-if-changed={}/package.json", web.display());
    println!("cargo::rerun-if-changed=build.rs");

    if std::env::var("GOAT_SKIP_WEB_BUILD").is_ok() {
        return;
    }
    if web.join("dist/index.html").exists() && std::env::var("PROFILE").as_deref() != Ok("release")
    {
        return;
    }

    let built = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&web)
        .status();

    match built {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("the web build exited with {status}"),
        Err(error) => panic!(
            "could not run npm in {}: {error}. Set GOAT_SKIP_WEB_BUILD to skip it.",
            web.display()
        ),
    }
}
