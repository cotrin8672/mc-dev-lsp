//! Test-only replacement for the legacy TeardownProcess.java fixture.
use std::{env, fs, io::Read, process::Command, thread, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.is_empty() || args == ["--child"] {
        thread::sleep(Duration::from_secs(60));
        return Ok(());
    }
    if args.len() != 2 || args[0] != "--mcdev-test-server" {
        return Err("usage: teardown_process [--child | --mcdev-test-server READY_FILE]".into());
    }
    // The child deliberately retains stdout/stderr after its parent exits.
    // Teardown must reap that owned child without killing an unrelated fixture.
    let child = Command::new(env::current_exe()?).arg("--child").spawn()?;
    fs::write(&args[1], child.id().to_string())?;
    std::io::stdin().read(&mut [0u8; 1])?;
    Ok(())
}
