#[tokio::main]
async fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] {
        println!("Usage: canopa-audio-provider [--protocol canopa-v5]");
        return;
    }
    if !args.is_empty() && args != ["--protocol", "canopa-v5"] {
        eprintln!("Usage: canopa-audio-provider [--protocol canopa-v5]");
        std::process::exit(2);
    }
    if let Err(error) = canopa_shell::audio_provider::run(
        tokio::io::BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
        None,
    )
    .await
    {
        eprintln!("canopa-audio-provider: {error}");
        std::process::exit(1);
    }
}
