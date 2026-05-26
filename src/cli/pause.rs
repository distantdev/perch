use crate::config::Config;
use crate::error::Result;
use crate::platform::PlatformScanner;
use crate::process::control::ProcessControl;

pub fn run_pause(target: &str, scanner: &dyn PlatformScanner, config: &Config) -> Result<()> {
    ProcessControl::pause(target, scanner, config)?;
    println!("Paused process for target {target}");
    Ok(())
}

pub fn run_resume(target: &str, scanner: &dyn PlatformScanner, config: &Config) -> Result<()> {
    ProcessControl::resume(target, scanner, config)?;
    println!("Resumed process for target {target}");
    Ok(())
}
