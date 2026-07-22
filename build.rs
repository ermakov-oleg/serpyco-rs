fn main() {
    pyo3_build_config::add_extension_module_link_args();
    pyo3_build_config::use_pyo3_cfgs();

    // pyo3 0.29+ links libpython on Windows via raw-dylib and no longer passes
    // the import library to the linker; our own extern for the private
    // `_PyDict_NewPresized` (src/python/py.rs) still needs it.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let config = pyo3_build_config::get();
        if let Some(lib_name) = config.lib_name() {
            println!("cargo:rustc-link-lib={lib_name}");
        }
        if let Some(lib_dir) = config.lib_dir() {
            println!("cargo:rustc-link-search=native={lib_dir}");
        }
    }
}
