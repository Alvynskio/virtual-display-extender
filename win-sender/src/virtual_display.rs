use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use crate::monitor;

/// Well-known registry path for the IddCx Virtual Display Driver.
const DRIVER_REGISTRY_PATH: &str =
    r"SYSTEM\CurrentControlSet\Enum\Root\DVDISPLAY";

/// Config directory used by the Virtual-Display-Driver (IddCx).
/// See: https://github.com/itsmikethetech/Virtual-Display-Driver
fn driver_config_dir() -> PathBuf {
    let program_data = std::env::var("ProgramData").unwrap_or_else(|_| r"C:\ProgramData".into());
    PathBuf::from(program_data).join("Virtual Display Driver")
}

/// Check whether an IddCx virtual display driver appears to be installed.
pub fn check_driver_installed() -> bool {
    // Simple heuristic: check if the driver's config directory exists.
    let dir = driver_config_dir();
    if dir.is_dir() {
        println!("[VirtualDisplay] Driver config directory found: {}", dir.display());
        return true;
    }

    // Also check the registry (best-effort, non-fatal on failure).
    #[cfg(windows)]
    {
        use std::process::Command;
        let output = Command::new("reg")
            .args(["query", DRIVER_REGISTRY_PATH])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                println!("[VirtualDisplay] Driver registry key found");
                return true;
            }
        }
    }

    false
}

/// Create a virtual monitor by writing the driver's config file and
/// triggering a device refresh.
///
/// Returns the monitor-index of the new virtual display, or an error.
pub fn create_virtual_monitor(width: u32, height: u32, hz: u32) -> Result<i32, String> {
    if !check_driver_installed() {
        return Err(
            "Virtual Display Driver not installed. \
             See https://github.com/itsmikethetech/Virtual-Display-Driver for installation."
                .into(),
        );
    }

    let monitors_before = monitor::list_monitors();

    // Write a config that adds one virtual display at the requested resolution.
    let config_dir = driver_config_dir();
    let config_path = config_dir.join("vdd_settings.xml");

    let config_content = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<VddConfig>
  <Monitors>
    <Monitor>
      <Width>{width}</Width>
      <Height>{height}</Height>
      <RefreshRate>{hz}</RefreshRate>
    </Monitor>
  </Monitors>
</VddConfig>"#,
    );

    fs::write(&config_path, &config_content)
        .map_err(|e| format!("Failed to write driver config at {}: {e}", config_path.display()))?;

    println!(
        "[VirtualDisplay] Wrote config: {}x{}@{}Hz to {}",
        width, height, hz, config_path.display()
    );

    // Trigger a device refresh so the driver picks up the new config.
    trigger_driver_refresh()?;

    // Wait for the new monitor to appear (up to 5 seconds).
    let index = find_new_monitor(&monitors_before, 5)?;
    println!("[VirtualDisplay] Virtual monitor created at index {index}");

    Ok(index)
}

/// Remove the virtual monitor by clearing the driver config and refreshing.
pub fn destroy_virtual_monitor() {
    let config_path = driver_config_dir().join("vdd_settings.xml");
    if config_path.exists() {
        let empty = r#"<?xml version="1.0" encoding="utf-8"?>
<VddConfig>
  <Monitors/>
</VddConfig>"#;
        let _ = fs::write(&config_path, empty);
        let _ = trigger_driver_refresh();
        println!("[VirtualDisplay] Virtual monitor removed");
    }
}

/// Trigger the IddCx driver to re-read its config by toggling the device.
fn trigger_driver_refresh() -> Result<(), String> {
    // Use devcon or pnputil to restart the device. Fall back to a simple
    // approach: disable then enable the device via PowerShell.
    let output = std::process::Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            r#"
$dev = Get-PnpDevice | Where-Object { $_.FriendlyName -like '*Virtual Display*' -or $_.FriendlyName -like '*IddSample*' } | Select-Object -First 1
if ($dev) {
    Disable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
    Enable-PnpDevice -InstanceId $dev.InstanceId -Confirm:$false -ErrorAction SilentlyContinue
}
"#,
        ])
        .output()
        .map_err(|e| format!("Failed to run PowerShell for driver refresh: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("[VirtualDisplay] Driver refresh warning: {stderr}");
    }

    Ok(())
}

/// Poll for a new monitor that wasn't in the before-list.
/// Returns the GStreamer monitor-index of the new display.
fn find_new_monitor(before: &[monitor::MonitorInfo], timeout_secs: u32) -> Result<i32, String> {
    let before_count = before.len();

    for _ in 0..(timeout_secs * 4) {
        thread::sleep(Duration::from_millis(250));
        let after = monitor::list_monitors();
        if after.len() > before_count {
            // The new monitor is the one whose name wasn't in the before list.
            let before_names: Vec<&str> = before.iter().map(|m| m.name.as_str()).collect();
            for m in &after {
                if !before_names.contains(&m.name.as_str()) {
                    return Ok(m.index as i32);
                }
            }
            // If names didn't help, just use the last index.
            return Ok((after.len() - 1) as i32);
        }
    }

    Err(format!(
        "Virtual monitor did not appear within {timeout_secs}s. \
         Check that the driver is installed and working."
    ))
}
