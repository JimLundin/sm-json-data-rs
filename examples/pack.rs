//! Writes a checkout of the data as a logic pack and reports its size.
//!
//! Run with: `cargo run --release --example pack -- <sm-json-data path> <output> [--docs]`

use sm_json_data::load_world;
use sm_logic::pack::{decode, encode, inspect, Docs, EncodeOptions};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: pack <sm-json-data path> <output> [--docs]";
    let root = args.next().ok_or(usage)?;
    let output = args.next().ok_or(usage)?;
    let docs = args.next().as_deref() == Some("--docs");

    let world = load_world(&root)?;
    let bytes = encode(&world, &EncodeOptions { docs, source: root })?;
    std::fs::write(&output, &bytes)?;

    let info = inspect(&bytes)?;
    println!("{output}: {} bytes ({} logic, {} docs)", bytes.len(), info.logic_bytes, info.docs_bytes);
    let started = Instant::now();
    decode(&bytes, Docs::Load)?;
    println!("decoded and validated in {:.1} ms", started.elapsed().as_secs_f64() * 1e3);
    Ok(())
}
