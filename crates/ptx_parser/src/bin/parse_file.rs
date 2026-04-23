//! Small CLI: read a PTX file or a C function signature from disk and
//! print the parsed result.
//!
//! Picks which parser to use based on file extension:
//!   *.ptx                          -> ptx_parser::parse (returns ParsedKernel)
//!   *.c, *.cpp, *.txt, other       -> ptx_parser::parse_c_signature (returns ParsedSignature)
//!
//! Usage:
//!   cargo run -p ptx_parser --bin parse_file -- path/to/file.ptx
//!   cargo run -p ptx_parser --bin parse_file -- path/to/signature.c

use std::env;
use std::fs;
use std::path::Path;
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

    let is_ptx = Path::new(path)
        .extension()
        .map(|e| e.eq_ignore_ascii_case("ptx"))
        .unwrap_or(false);

    if is_ptx {
        print_ptx(&path, &input);
    } else {
        print_c_signature(&path, &input);
    }
}

fn print_ptx(path: &str, input: &str) {
    match ptx_parser::parse(input) {
        Ok(kernel) => {
            println!("========================================");
            println!("  Parsed: {} (PTX)", path);
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
            println!("instructions ({} total):", kernel.instructions.len());
            for (pc, inst) in kernel.instructions.iter().enumerate() {
                println!("  [{pc:3}] {:?} args={:?}", inst.inst_type, inst.args);
            }
        }
        Err(e) => {
            eprintln!("Parse error: {e}");
            process::exit(1);
        }
    }
}

fn print_c_signature(path: &str, input: &str) {
    match ptx_parser::parse_c_signature(input) {
        Ok(sig) => {
            println!("========================================");
            println!("  Parsed: {} (C signature)", path);
            println!("========================================");
            println!("function name: {}", sig.name);
            println!();
            println!("params ({} total):", sig.params.len());
            for (i, p) in sig.params.iter().enumerate() {
                let display_name = if p.name.is_empty() {
                    "<unnamed>"
                } else {
                    &p.name
                };
                // Render pointer levels as a visible "*" string so the
                // output reads like the original source ("float*").
                let stars: String = "*".repeat(p.pointer_levels as usize);
                println!(
                    "  [{i}] {} : {:?}{}  (pointer_levels = {})",
                    display_name, p.ptx_type, stars, p.pointer_levels
                );
            }
        }
        Err(e) => {
            eprintln!("Parse error: {e}");
            process::exit(1);
        }
    }
}