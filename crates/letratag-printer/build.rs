fn main() {
    println!("cargo:rerun-if-changed=native/adapter.c");
    println!("cargo:rerun-if-changed=native/adapter.h");
    println!("cargo:rerun-if-changed=../../patches/pappl-1.4.9-document-policy.patch");
    println!("cargo:rerun-if-env-changed=LETRATAG_PAPPL_PREFIX");
    if std::env::var_os("CARGO_FEATURE_PAPPL").is_some() {
        let mut config = pkg_config::Config::new();
        config.atleast_version("1.4.9");
        if let Ok(prefix) = std::env::var("LETRATAG_PAPPL_PREFIX") {
            config.arg(format!("--define-variable=prefix={prefix}"));
        }
        let lib = config
            .probe("pappl")
            .expect("PAPPL >= 1.4.9 is required; see scripts/build-pappl.sh");
        for directory in &lib.link_paths {
            let archive = directory.join("libpappl.a");
            if archive.exists() {
                println!("cargo:rerun-if-changed={}", archive.display());
            }
        }
        let mut c = cc::Build::new();
        c.file("native/adapter.c").flag_if_supported("-std=c11");
        for include in lib.include_paths {
            c.include(include);
        }
        c.compile("letratag_pappl_adapter");
    }
}
