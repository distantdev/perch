use crate::config::Config;
use crate::error::Result;
use crate::platform::PlatformScanner;
use crate::process::control::ProcessControl;

pub fn run_kill(
    target: &str,
    force: bool,
    scanner: &dyn PlatformScanner,
    config: &Config,
) -> Result<()> {
    ProcessControl::kill(target, force, scanner, config)?;
    println!("Stopped process for target {target}");
    Ok(())
}
