//! Root helper for Settings: set or remove the login screen's display layout.
//! Installed as `/usr/lib/canopa/greeter-displays` and run through `pkexec`.
fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = canopa_greeter::displays::main(&arguments, std::io::stdin().lock()) {
        eprintln!("canopa-greeter-displays: {error:#}");
        std::process::exit(1);
    }
}
