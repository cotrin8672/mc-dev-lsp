//! Test-only transparent recorder; all files live in the explicit test log dir.
use std::{fs::{self,File},io::{self,Read,Write},path::PathBuf,process::{Command,Stdio},thread};
fn relay(mut input:impl Read,mut output:impl Write,mut recording:File)->io::Result<()>{let mut bytes=[0;8192];loop{let n=input.read(&mut bytes)?;if n==0{return Ok(())}recording.write_all(&bytes[..n])?;recording.flush()?;output.write_all(&bytes[..n])?;output.flush()?;}}
fn main()->io::Result<()>{
    let logs=PathBuf::from(std::env::var_os("MCDEV_WIRE_LOG_DIR").expect("explicit test logs"));
    let logs=logs.join(std::process::id().to_string());fs::create_dir_all(&logs)?;
    let child=std::env::var_os("MCDEV_WIRE_CHILD").expect("explicit immutable child");
    let mut child=Command::new(child).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    fs::write(logs.join("pid.txt"),child.id().to_string())?;
    let input=child.stdin.take().unwrap();let output=child.stdout.take().unwrap();let stderr=child.stderr.take().unwrap();
    let incoming=File::create(logs.join("host-to-child.bin"))?;let outgoing=File::create(logs.join("child-to-host.bin"))?;let errors=File::create(logs.join("child.stderr.log"))?;
    thread::spawn(move||relay(io::stdin(),input,incoming));
    let out=thread::spawn(move||relay(output,io::stdout(),outgoing));
    let err=thread::spawn(move||relay(stderr,io::sink(),errors));
    let status=child.wait()?;let stdout=out.join().unwrap();let stderr=err.join().unwrap();
    fs::write(logs.join("exit.txt"),format!("status={status};stdout={stdout:?};stderr={stderr:?}"))?;
    std::process::exit(status.code().unwrap_or(1));
}
