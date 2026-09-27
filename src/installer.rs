use anyhow::{Context, Result};
use std::env;
use std::fs;
use std::path::PathBuf;

/// Installs the smartxfer binary to the user's home directory and adds it to the User PATH
pub fn install_binary() -> Result<()> {
    let current_exe = env::current_exe()
        .context("Failed to get current executable path")?;

    let home_dir = dirs_fallback();
    let install_dir = home_dir.join(".smartxfer").join("bin");

    fs::create_dir_all(&install_dir)
        .with_context(|| format!("Failed to create directory {:?}", install_dir))?;

    let target_smartxfer = install_dir.join("smartxfer.exe");
    let target_durasend = install_dir.join("durasend.exe");

    println!("[installer] Copying binary to {:?}...", target_smartxfer);
    fs::copy(&current_exe, &target_smartxfer)
        .with_context(|| format!("Failed to copy binary to {:?}", target_smartxfer))?;

    // Also copy as durasend.exe alias
    let _ = fs::copy(&current_exe, &target_durasend);

    println!("[installer] Binary installed successfully.");

    // Add to Windows User PATH
    if cfg!(target_os = "windows") {
        add_to_windows_user_path(&install_dir)?;
    } else {
        println!("[installer] On Linux/macOS, add this directory to your shell profile (~/.bashrc or ~/.zshrc):");
        println!("  export PATH=\"$PATH:{}\"", install_dir.display());
    }

    println!();
    println!("[SUCCESS] Installation Complete.");
    println!("You can now open any new terminal window and type:");
    println!("   smartxfer --help");
    println!("   smartxfer ui");
    println!();

    Ok(())
}

fn dirs_fallback() -> PathBuf {
    if let Ok(userprofile) = env::var("USERPROFILE") {
        PathBuf::from(userprofile)
    } else if let Ok(home) = env::var("HOME") {
        PathBuf::from(home)
    } else {
        env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }
}

fn add_to_windows_user_path(install_dir: &std::path::Path) -> Result<()> {
    let install_dir_str = install_dir.to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid install path string"))?;

    // Use powershell to check and update User environment Path cleanly
    let ps_command = format!(
        "$dir = '{}'; \
         $currentPath = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         if ($currentPath -notlike \"*$dir*\") {{ \
             $newPath = if ($currentPath) {{ \"$currentPath;$dir\" }} else {{ $dir }}; \
             [Environment]::SetEnvironmentVariable('Path', $newPath, 'User'); \
             Write-Output 'ADDED' \
         }} else {{ \
             Write-Output 'ALREADY_EXISTS' \
         }}",
        install_dir_str.replace("'", "''")
    );

    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &ps_command])
        .output();

    match output {
        Ok(out) => {
            let res = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if res.contains("ADDED") {
                println!("[installer] Added {:?} to your Windows User PATH environment variable.", install_dir);
            } else {
                println!("[installer] {:?} is already present in your PATH.", install_dir);
            }
        }
        Err(e) => {
            println!("[installer] Could not automatically update PATH: {}", e);
            println!("[installer] Please add {:?} to your System/User PATH manually.", install_dir);
        }
    }

    Ok(())
}
