use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub(super) struct P4Config {
    pub(super) client: Option<String>,
    pub(super) port: Option<String>,
    pub(super) user: Option<String>,
    pub(super) charset: Option<String>,
    pub(super) source_path: Option<PathBuf>,
}

pub(super) fn load_p4_config(cwd: Option<&str>) -> P4Config {
    let Some(cwd) = cwd.filter(|value| !value.trim().is_empty()) else {
        return P4Config::default();
    };

    let mut current = PathBuf::from(cwd);
    loop {
        let candidate = current.join(".p4config");
        if candidate.is_file() {
            let mut config = P4Config {
                source_path: Some(candidate.clone()),
                ..P4Config::default()
            };
            if let Ok(contents) = std::fs::read_to_string(&candidate) {
                for raw_line in contents.lines() {
                    let line = raw_line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    let Some((key, value)) = line.split_once('=') else {
                        continue;
                    };
                    let value = value.trim().to_string();
                    match key.trim() {
                        "P4CLIENT" => config.client = Some(value),
                        "P4PORT" => config.port = Some(value),
                        "P4USER" => config.user = Some(value),
                        "P4CHARSET" => config.charset = Some(value),
                        _ => {}
                    }
                }
            }
            return config;
        }
        if !current.pop() {
            break;
        }
    }

    P4Config::default()
}

pub(super) fn apply_p4_config_env(cmd: &mut Command, p4_config: &P4Config) {
    if let Some(client) = p4_config
        .client
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        cmd.env("P4CLIENT", client);
    }
    if let Some(port) = p4_config
        .port
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        cmd.env("P4PORT", port);
    }
    if let Some(user) = p4_config
        .user
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        cmd.env("P4USER", user);
    }
    let charset = p4_config
        .charset
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("utf8");
    cmd.env("P4CHARSET", charset);
}
