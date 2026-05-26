use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::debugging::DebugLogger;

use super::p4_config::{apply_p4_config_env, P4Config};

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub(crate) fn run_command(
    program: &str,
    args: &[&str],
    cwd: Option<&str>,
) -> Result<String, String> {
    let args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    run_command_owned(program, &args, cwd, None)
}

pub(crate) fn run_p4(
    args: &[&str],
    cwd: Option<&str>,
    p4_config: &P4Config,
) -> Result<String, String> {
    let owned = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    run_p4_owned(&owned, cwd, p4_config)
}

pub(crate) fn run_p4_owned(
    args: &[String],
    cwd: Option<&str>,
    p4_config: &P4Config,
) -> Result<String, String> {
    run_command_owned("p4", args, cwd, Some(p4_config))
}

fn run_command_owned(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    p4_config: Option<&P4Config>,
) -> Result<String, String> {
    let debug = DebugLogger::new("scm");
    if program == "p4" {
        debug.log(format!(
            "command={} cwd={:?} args={} client={:?} port={:?} user={:?} config_path={:?}",
            program,
            cwd,
            args.join(" "),
            p4_config.and_then(|config| config.client.as_deref()),
            p4_config.and_then(|config| config.port.as_deref()),
            p4_config.and_then(|config| config.user.as_deref()),
            p4_config.and_then(|config| config.source_path.as_ref())
        ));
    }

    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd.args(args);
    if let Some(cwd) = cwd.filter(|value| !value.trim().is_empty()) {
        cmd.current_dir(cwd);
    }
    if let Some(p4_config) = p4_config {
        apply_p4_config_env(&mut cmd, p4_config);
    }

    let output = cmd
        .output()
        .map_err(|err| format!("Failed to run {}: {}", program, err))?;
    if program == "p4" {
        debug.log(format!("status={}", output.status));
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        debug.log_multiline("stdout", &stdout);
        debug.log_multiline("stderr", &stderr);
    }
    if !output.status.success() {
        return Err(format!(
            "{} {} failed: {}",
            program,
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
