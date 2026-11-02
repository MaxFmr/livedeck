//! Replays raw MIDI bytes through the decoder and `config/mapping.toml`, printing the actions.
//! cargo run -p io --example replay -- 90037F 97397F 973900 900300
use io::controller::Decoder;
use io::mapping::Mapping;

fn main() {
    let mut mapping = Mapping::load(std::path::Path::new("config/mapping.toml")).expect("mapping");
    let mut decoder = Decoder::new();
    for (t, hex) in std::env::args().skip(1).enumerate() {
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .filter_map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect();
        let event = decoder.decode(&bytes);
        let action = event.and_then(|e| mapping.handle(t as u64 * 1_000, e));
        println!("{hex} → {event:?} → {action:?}");
    }
}
