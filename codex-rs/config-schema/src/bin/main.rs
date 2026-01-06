//! CLI tool to generate JSON Schema for config.toml.

use std::path::PathBuf;

use codex_config_schema::SchemaConfig;
use codex_config_schema::generate_config_schema;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    // Simple arg parsing: -o <file> or --out <file>
    let out_file = parse_output_arg(&args)?;

    let config = SchemaConfig::from_git();
    let schema = generate_config_schema(&config);
    let json = serde_json::to_string_pretty(&schema)?;

    std::fs::write(&out_file, json)?;
    eprintln!("Wrote config schema to {}", out_file.display());

    Ok(())
}

fn parse_output_arg(args: &[String]) -> Result<PathBuf, String> {
    let program_name = args.first().map_or("program", String::as_str);
    let mut iter = args.iter().skip(1); // Skip program name
    while let Some(arg) = iter.next() {
        if arg == "-o" || arg == "--out" {
            if let Some(path) = iter.next() {
                return Ok(PathBuf::from(path));
            }
            return Err("Missing output file path after -o/--out".to_string());
        }
    }
    Err(format!("Usage: {program_name} -o <output-file>"))
}
