fn main() {
    println!("cargo:rerun-if-changed=assets/nen.rc");
    println!("cargo:rerun-if-changed=assets/nen.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
                .join("nen.res");
        let status=std::process::Command::new("rc.exe").arg("/nologo").arg("/fo").arg(&out).arg("assets/nen.rc").status().expect("Windows SDK rc.exe is required. Run scripts/cargo.ps1 or use a Visual Studio developer shell.");
        assert!(status.success(), "Windows resource compilation failed");
        println!("cargo:rustc-link-arg-bins={}", out.display());
    }
}
