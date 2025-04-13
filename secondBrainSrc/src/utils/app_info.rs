use anyhow::Result;

#[cfg(target_os = "macos")]
pub fn get_active_app() -> Result<String> {
    // On macOS, we would use the Objective-C bridge or a command line tool
    // This is a simplified mock version
    // In a real implementation, you'd use a crate like objc-foundation or cocoa-rs
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("osascript -e 'tell application \"System Events\" to get name of first application process whose frontmost is true'")
        .output()?;
        
    let app_name = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
        
    Ok(app_name)
}

#[cfg(target_os = "windows")]
pub fn get_active_app() -> Result<String> {
    // On Windows, this would use the Windows API
    // Simplified mock implementation
    Ok("Windows App".to_string())
}

#[cfg(target_os = "linux")]
pub fn get_active_app() -> Result<String> {
    // On Linux, we would use X11 or Wayland API
    // Simplified mock implementation
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("xdotool getwindowfocus getwindowname")
        .output()?;
        
    let app_name = String::from_utf8(output.stdout)?
        .trim()
        .to_string();
        
    Ok(app_name)
}
