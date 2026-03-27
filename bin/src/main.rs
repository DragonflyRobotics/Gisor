use std::{
    env,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Command, Stdio},
};

fn main() {
    println!("Running all tests with emulator :)");
    // print current directory
    let binary_path: PathBuf = env::current_dir().unwrap().join("bin").join("out");
    for binary in binary_path.read_dir().unwrap() {
        let binary = binary.unwrap();
        println!(
            "=============================={}=============================",
            binary.file_name().to_str().unwrap()
        );
        let mut output = Command::new(binary.path())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute process");
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
        println!(
            "=============================={}=============================",
            binary.file_name().to_str().unwrap()
        );
    }
}
