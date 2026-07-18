//! Repo chores runner. First resident: the icon pipeline
//! (`cargo run -p xtask -- icons`) — master SVG to ico/icns/hicolor.
//! Implementation lands with the packaging round (W4).

fn main() {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "icons" => todo!("icon pipeline lands with the packaging round"),
        _ => eprintln!("usage: cargo run -p xtask -- icons"),
    }
}
