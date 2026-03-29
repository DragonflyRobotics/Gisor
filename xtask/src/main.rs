use std::env;
use std::process::{Command, ExitCode};

fn run_command(mut cmd: Command, printable: &str) -> Result<(), String> {
    eprintln!("> {printable}");
    let status = cmd
        .status()
        .map_err(|e| format!("failed to start `{printable}`: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command failed: {printable}"))
    }
}

fn build_gisor(release: bool) -> Result<(), String> {
    let mut cmd = Command::new("cargo");
    cmd.arg("build").arg("-p").arg("gisor").arg("--lib");
    if release {
        cmd.arg("--release");
    }
    run_command(
        cmd,
        if release {
            "cargo build -p gisor --lib --release"
        } else {
            "cargo build -p gisor --lib"
        },
    )
}

fn main() -> ExitCode {
    let argv: Vec<String> = env::args().skip(1).collect();
    let sub = argv.get(0).map(String::as_str).unwrap_or("help");

    // xtask flags + forwarded args
    let rest = &argv[1..];
    let split = rest.iter().position(|a| a == "--");
    let (xtask_args, forwarded) = match split {
        Some(i) => (&rest[..i], &rest[i + 1..]),
        None => (rest, &[][..]),
    };

    let release = xtask_args.iter().any(|a| a == "--release");

    let result = match sub {
        "launch" => {
            if let Err(e) = build_gisor(release) {
                return { eprintln!("{e}"); ExitCode::FAILURE };
            }

            let mut cmd = Command::new("cargo");
            cmd.arg("run")
                .arg("-p")
                .arg("gisor_launch")
                .arg("--bin")
                .arg("gisor_launch"); // <-- change if your actual bin name differs

            if release {
                cmd.arg("--release");
            }

            if !forwarded.is_empty() {
                cmd.arg("--");
                cmd.args(forwarded);
            }

            run_command(cmd, "cargo run -p gisor_launch --bin tests ...")
        }
        _ => {
            eprintln!("xtask usage:");
            eprintln!("  cargo run -p xtask -- launch [--release] -- <args for gisor_launch bin>");
            Ok(())
        }
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}
