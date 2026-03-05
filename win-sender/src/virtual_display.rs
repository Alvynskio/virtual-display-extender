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

/// Extract a value from a simple XML tag like `<Tag>value</Tag>`.
fn extract_xml_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim().to_string())
}

/// Information about a detected virtual display from the driver config.
#[derive(Debug, Clone)]
pub struct VirtualDisplayInfo {
    /// Monitor index in the system monitor list.
    pub index: i32,
    /// Configured width.
    pub width: u32,
    /// Configured height.
    pub height: u32,
    /// Configured refresh rate.
    pub refresh_rate: u32,
}

/// Detect an existing virtual display by reading the driver config and
/// cross-referencing with active monitors.
///
/// Returns `Some(info)` if a virtual display is currently active, `None` otherwise.
pub fn detect_existing_virtual_display() -> Option<VirtualDisplayInfo> {
    let config_path = driver_config_dir().join("vdd_settings.xml");
    let xml = fs::read_to_string(&config_path).ok()?;

    // Check if there's actually a monitor configured (not an empty <Monitors/> tag).
    if !xml.contains("<Monitor>") {
        return None;
    }

    let width: u32 = extract_xml_value(&xml, "Width")?.parse().ok()?;
    let height: u32 = extract_xml_value(&xml, "Height")?.parse().ok()?;
    let refresh_rate: u32 = extract_xml_value(&xml, "RefreshRate")?.parse().ok()?;

    // Cross-reference with active monitors to find the virtual display.
    let monitors = monitor::list_monitors();
    for m in &monitors {
        // Virtual displays from IddCx typically don't have a primary flag
        // and their name often differs from physical displays.
        // Match by resolution since we know the configured size.
        if m.width == width && m.height == height && !m.primary {
            println!(
                "[VirtualDisplay] Found existing virtual display: {} (index {}, {}x{}@{}Hz)",
                m.name, m.index, width, height, refresh_rate,
            );
            return Some(VirtualDisplayInfo {
                index: m.index as i32,
                width,
                height,
                refresh_rate,
            });
        }
    }

    // Config exists but no matching monitor is active — driver may be disabled.
    println!(
        "[VirtualDisplay] Config found ({}x{}@{}Hz) but no matching active monitor",
        width, height, refresh_rate,
    );
    None
}

/// Create a virtual monitor, or reuse an existing one if it matches the
/// requested resolution. If a virtual display exists but with a different
/// resolution, destroy it first and recreate.
///
/// Returns the monitor-index of the virtual display.
pub fn create_or_reuse_virtual_monitor(width: u32, height: u32, hz: u32) -> Result<i32, String> {
    if let Some(existing) = detect_existing_virtual_display() {
        if existing.width == width && existing.height == height {
            println!(
                "[VirtualDisplay] Reusing existing virtual display at index {} ({}x{})",
                existing.index, existing.width, existing.height,
            );
            return Ok(existing.index);
        }

        // Mismatched resolution — destroy and recreate.
        println!(
            "[VirtualDisplay] Existing display is {}x{}, need {}x{} — recreating",
            existing.width, existing.height, width, height,
        );
        destroy_virtual_monitor();
    }

    create_virtual_monitor(width, height, hz)
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
/// Polls for up to 3 seconds to confirm the monitor was removed.
pub fn destroy_virtual_monitor() {
    let config_path = driver_config_dir().join("vdd_settings.xml");
    if !config_path.exists() {
        println!("[VirtualDisplay] No config file found, nothing to destroy");
        return;
    }

    let monitors_before = monitor::list_monitors();
    let count_before = monitors_before.len();

    let empty = r#"<?xml version="1.0" encoding="utf-8"?>
<VddConfig>
  <Monitors/>
</VddConfig>"#;
    let _ = fs::write(&config_path, empty);
    let _ = trigger_driver_refresh();

    // Poll for monitor removal confirmation (up to 3 seconds).
    for i in 0..12 {
        thread::sleep(Duration::from_millis(250));
        let count_now = monitor::list_monitors().len();
        if count_now < count_before {
            println!(
                "[VirtualDisplay] Virtual monitor removed ({}ms, {} → {} monitors)",
                (i + 1) * 250,
                count_before,
                count_now,
            );
            return;
        }
    }

    println!(
        "[VirtualDisplay] Config cleared but monitor count unchanged ({count_before}). \
         Driver may need manual restart."
    );
}

/// Print the current status of the virtual display driver and any active
/// virtual display.
pub fn print_status() {
    println!("=== Virtual Display Driver Status ===\n");

    // Driver installation check.
    let installed = check_driver_installed();
    println!("Driver installed: {}", if installed { "yes" } else { "NO" });

    // Config file contents.
    let config_path = driver_config_dir().join("vdd_settings.xml");
    if config_path.exists() {
        println!("Config file: {}", config_path.display());
        if let Ok(contents) = fs::read_to_string(&config_path) {
            println!("Config contents:\n{contents}");
        }
    } else {
        println!("Config file: not found");
    }

    println!();

    // Active monitors.
    let monitors = monitor::list_monitors();
    monitor::print_monitors(&monitors);

    println!();

    // Virtual display detection.
    match detect_existing_virtual_display() {
        Some(info) => {
            println!(
                "Active virtual display: index {}, {}x{}@{}Hz",
                info.index, info.width, info.height, info.refresh_rate,
            );
        }
        None => {
            println!("Active virtual display: none detected");
        }
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
