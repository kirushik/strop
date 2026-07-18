fn main() {
    let result = match std::env::args().nth(1).as_deref() {
        Some("icons") => xtask::icons(
            "assets/icon/strop-mark.svg",
            "packaging/generated",
        ),
        _ => {
            eprintln!("usage: cargo run -p xtask -- icons");
            std::process::exit(2);
        }
    };
    if let Err(error) = result {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}
