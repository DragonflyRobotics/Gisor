//! Small CLI: read a PTX file or a C function signature from disk and
//! print the parsed result.
//!
//! Format is auto-detected. Usage:
//!   cargo run -p ptx_parser --bin parse_file -- path/to/file.ptx
//!   cargo run -p ptx_parser --bin parse_file -- path/to/signature.txt

use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path-to-ptx-or-signature-file>", args[0]);
        process::exit(1);
    }

    let path = &args[1];
    let input = match fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {path}: {e}");
            process::exit(1);
        }
    };

    match ptx_parser::parse(&input) {
        Ok(kernel) => {
            println!("========================================");
            println!("  Parsed: {}", path);
            println!("========================================");
            println!("kernel name: {}", kernel.name);
            println!();
            println!("params ({} total):", kernel.params.len());
            for (i, p) in kernel.params.iter().enumerate() {
                let display_name = if p.name.is_empty() {
                    "<unnamed>"
                } else {
                    &p.name
                };
                println!("  [{i}] {} : {:?}", display_name, p.ptx_type);
            }
            println!();
            if kernel.instructions.is_empty() {
                println!("instructions: none (input was a C signature, no PTX body)");
            } else {
                println!("instructions ({} total):", kernel.instructions.len());
                for (pc, inst) in kernel.instructions.iter().enumerate() {
                    println!("  [{pc:3}] {:?} args={:?}", inst.inst_type, inst.args);
                }
            }
        }
        Err(e) => {
            eprintln!("Parse error: {e}");
            process::exit(1);
        }
    }
}