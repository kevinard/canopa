fn main() {
    println!("cargo:rerun-if-changed=native/agent.c");
    let library = pkg_config::Config::new()
        .atleast_version("0.105")
        .probe("polkit-agent-1")
        .expect("install polkit-agent-1 development headers");
    let mut build = cc::Build::new();
    build
        .file("native/agent.c")
        .warnings(true)
        .flag("-Wall")
        .flag("-Wextra")
        .flag("-Werror");
    for path in library.include_paths {
        build.include(path);
    }
    build.compile("canopa_polkit_bridge");
}
