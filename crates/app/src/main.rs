//! LiveDeck application: UI (eframe/egui) and thread wiring.

fn main() {
    println!(
        "LiveDeck {} — reference block budget: {:.0} µs",
        env!("CARGO_PKG_VERSION"),
        engine::block_budget_us(128, 48_000)
    );
}
