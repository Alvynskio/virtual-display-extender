use std::env;
use std::path::PathBuf;

/// Create a Start Menu shortcut (.lnk) pointing to the current executable
/// with `--tray` flag.
pub fn install_start_menu_shortcut() -> Result<PathBuf, String> {
    let exe_path = env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {e}"))?;

    let appdata = env::var("APPDATA")
        .map_err(|_| "APPDATA environment variable not set".to_string())?;

    let start_menu_dir = PathBuf::from(&appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Start Menu")
        .join("Programs");

    let lnk_path = start_menu_dir.join("Virtual Display Extender.lnk");

    // Use PowerShell to create the .lnk file.
    let ps_script = format!(
        r#"
$ws = New-Object -ComObject WScript.Shell
$shortcut = $ws.CreateShortcut("{lnk}")
$shortcut.TargetPath = "{exe}"
$shortcut.Arguments = "--tray"
$shortcut.Description = "Virtual Display Extender - Stream virtual display over network"
$shortcut.WorkingDirectory = "{dir}"
$shortcut.Save()
"#,
        lnk = lnk_path.display(),
        exe = exe_path.display(),
        dir = exe_path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_script])
        .output()
        .map_err(|e| format!("Failed to run PowerShell: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("PowerShell shortcut creation failed: {stderr}"));
    }

    println!("[Shortcut] Created: {}", lnk_path.display());
    Ok(lnk_path)
}
