//! One-off helper: read a `.path.json` file, write the JSONL form to a
//! `.path.jsonl` file at the same basename. Used to regenerate the JSONL
//! example files under `examples/` whenever their JSON counterparts change.
//!
//! Usage: `cargo run -p path-cli --example convert-path-to-jsonl -- <in.path.json> <out.path.jsonl>`

use std::fs;
use toolpath::v1::Graph;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let input = args.next().expect("usage: <in.path.json> <out.path.jsonl>");
    let output = args.next().expect("usage: <in.path.json> <out.path.jsonl>");

    let json = fs::read_to_string(&input)?;
    let graph = Graph::from_json(&json)?;
    let jsonl = graph.to_jsonl_string()?;
    fs::write(&output, jsonl)?;
    println!("wrote {output}");
    Ok(())
}
