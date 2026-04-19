use std::{
    env, fs,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

fn main() {
    let args: Vec<String> = env::args().collect();
    assert_eq!(args.len(), 3);
    let run_file = &args[1];
    let ptx_file = &args[2];
    assert!(run_file.ends_with(".run"));
    assert!(ptx_file.ends_with(".ptx"));
    let ptx = fs::read_to_string(ptx_file).ok().unwrap();
    unsafe { env::set_var("GISOR_PTX", ptx) };
    let ld_path = std::env::current_exe().unwrap();
    let mut output = match Command::new(format!("{}", run_file))
        .env(
            "LD_PRELOAD",
            ld_path
                .parent()
                .unwrap()
                .join("libgisor.so")
                .to_str()
                .unwrap(),
        )
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => panic!("failed to execute process: {}", e),
    };
    if let Some(stdout) = output.stdout.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            println!("+++ {}", line.unwrap());
        }
    }
    if let Some(stdout) = output.stderr.take() {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            println!("--- {}", line.unwrap());
        }
    }
}
