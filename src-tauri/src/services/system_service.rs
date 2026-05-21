use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn browse_for_directory(initial_path: Option<&str>) -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        let script = r#"
Add-Type -AssemblyName System.Windows.Forms
$dialog = New-Object System.Windows.Forms.FolderBrowserDialog
$dialog.ShowNewFolderButton = $false
$initial = $env:DIFFVIEWER_INITIAL_DIR
if ($initial -and (Test-Path -LiteralPath $initial)) {
  $dialog.SelectedPath = $initial
}
$result = $dialog.ShowDialog()
if ($result -eq [System.Windows.Forms.DialogResult]::OK) {
  Write-Output $dialog.SelectedPath
}
"#;

        let mut command = Command::new("powershell");
        command.creation_flags(CREATE_NO_WINDOW);
        command.args(["-NoProfile", "-STA", "-Command", script]);
        if let Some(initial_path) = initial_path.filter(|value| !value.trim().is_empty()) {
            command.env("DIFFVIEWER_INITIAL_DIR", initial_path);
        }

        let output = command
            .output()
            .map_err(|err| format!("Failed to open folder picker: {}", err))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
        }

        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() {
            Ok(None)
        } else {
            Ok(Some(selected))
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = initial_path;
        Err("Directory picker is not implemented for this platform yet".to_string())
    }
}
