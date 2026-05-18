use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpenRequest {
    OpenPatch {
        path: String,
    },
    OpenTwoWay {
        left_path: String,
        right_path: String,
        title: Option<String>,
        left_label: Option<String>,
        right_label: Option<String>,
    },
    OpenMerge {
        base_path: String,
        local_path: String,
        remote_path: String,
        merged_path: String,
        title: Option<String>,
    },
    OpenFiles {
        paths: Vec<String>,
    },
}

/// Parse command-line arguments into an OpenRequest.
pub fn parse_argv(args: &[String]) -> Option<OpenRequest> {
    if args.len() < 2 {
        return None;
    }

    // Skip executable name and sanitize away cargo/tauri passthrough args.
    let raw = &args[1..];
    let mut cleaned: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < raw.len() {
        let arg = &raw[i];
        match arg.as_str() {
            "--" => {
                i += 1;
            }
            "--debug" => {
                i += 1;
            }
            "--no-default-features" | "--release" => {
                i += 1;
            }
            "--color" => {
                // Consume --color <when>
                i += if i + 1 < raw.len() { 2 } else { 1 };
            }
            _ => {
                cleaned.push(arg.clone());
                i += 1;
            }
        }
    }
    let args = cleaned;
    let args = args.as_slice();

    // --diff leftPath rightPath
    if args.len() >= 3 && args[0] == "--diff" {
        return Some(OpenRequest::OpenTwoWay {
            left_path: args[1].clone(),
            right_path: args[2].clone(),
            title: None,
            left_label: None,
            right_label: None,
        });
    }

    // --merge --base B --local L --remote R --merged M
    if args.contains(&"--merge".to_string()) {
        let get = |flag: &str| -> Option<String> {
            args.iter()
                .position(|a| a == flag)
                .and_then(|i| args.get(i + 1))
                .cloned()
        };
        if let (Some(base), Some(local), Some(remote), Some(merged)) = (
            get("--base"),
            get("--local"),
            get("--remote"),
            get("--merged"),
        ) {
            return Some(OpenRequest::OpenMerge {
                base_path: base,
                local_path: local,
                remote_path: remote,
                merged_path: merged,
                title: None,
            });
        }
    }

    // --open <path> (patch)
    if args.len() >= 2 && args[0] == "--open" {
        return Some(OpenRequest::OpenPatch {
            path: args[1].clone(),
        });
    }

    // Single file ending in .diff or .patch
    if args.len() == 1 {
        let p = &args[0];
        if p.ends_with(".diff") || p.ends_with(".patch") {
            return Some(OpenRequest::OpenPatch { path: p.clone() });
        }
    }

    // Two bare paths become a two-way compare.
    if args.len() == 2 {
        return Some(OpenRequest::OpenTwoWay {
            left_path: args[0].clone(),
            right_path: args[1].clone(),
            title: None,
            left_label: None,
            right_label: None,
        });
    }

    // Multiple files become OpenFiles (drag/drop).
    if !args.is_empty() {
        return Some(OpenRequest::OpenFiles {
            paths: args.to_vec(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_debug_flag_when_parsing_argv() {
        let args = vec![
            "diffviewer.exe".to_string(),
            "--debug".to_string(),
            "--diff".to_string(),
            "left.txt".to_string(),
            "right.txt".to_string(),
        ];

        let request = parse_argv(&args);
        assert!(matches!(
            request,
            Some(OpenRequest::OpenTwoWay { left_path, right_path, .. })
            if left_path == "left.txt" && right_path == "right.txt"
        ));
    }
}
