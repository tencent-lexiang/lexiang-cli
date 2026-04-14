use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct RamdiskConfig {
    pub name: String,
    pub size_mb: u64,
    pub mount_point: PathBuf,
}

impl Default for RamdiskConfig {
    fn default() -> Self {
        Self {
            name: "lexiang-ramdisk".to_string(),
            size_mb: 256,
            mount_point: PathBuf::from("/tmp/lexiang"),
        }
    }
}

pub struct Ramdisk;

impl Ramdisk {
    /// Create a ramdisk (platform-specific)
    pub fn create(config: &RamdiskConfig) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            Self::create_macos(config)
        }

        #[cfg(target_os = "linux")]
        {
            Self::create_linux(config)
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            anyhow::bail!("Unsupported platform for ramdisk");
        }
    }

    /// Unmount and destroy ramdisk
    pub fn destroy(mount_point: &std::path::Path) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            Self::destroy_macos(mount_point)
        }

        #[cfg(target_os = "linux")]
        {
            Self::destroy_linux(mount_point)
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            anyhow::bail!("Unsupported platform for ramdisk");
        }
    }

    /// Check if ramdisk is mounted
    pub fn is_mounted(mount_point: &std::path::Path) -> bool {
        mount_point.exists() && mount_point.is_dir()
    }

    #[cfg(target_os = "macos")]
    fn create_macos(config: &RamdiskConfig) -> Result<()> {
        use std::process::Command;

        // Calculate size in 512-byte sectors
        let sectors = config.size_mb * 2048;

        // Create ramdisk device
        let output = Command::new("hdiutil")
            .args(["attach", "-nomount", "ram://", &sectors.to_string()])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to create ramdisk: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let device = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Format as HFS+
        let output = Command::new("diskutil")
            .args(["eraseVolume", "HFS+", &config.name, &device])
            .output()?;

        if !output.status.success() {
            // Cleanup on failure
            let _ = Command::new("hdiutil").args(["detach", &device]).output();
            anyhow::bail!(
                "Failed to format ramdisk: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Move to desired location if not default
        let default_mount = PathBuf::from(format!("/Volumes/{}", config.name));
        if default_mount != config.mount_point {
            std::fs::create_dir_all(&config.mount_point)?;
            std::fs::rename(&default_mount, &config.mount_point)?;
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn create_linux(config: &RamdiskConfig) -> Result<()> {
        // Use /dev/shm (POSIX shared memory) which is a proper memory-based filesystem
        // /dev/shm is backed by RAM and doesn't swap to disk like tmpfs can
        let shm_path = PathBuf::from("/dev/shm").join(&config.name);

        // Create directory in /dev/shm
        std::fs::create_dir_all(&shm_path)?;

        // Create symlink from mount_point to shm_path
        if config.mount_point != shm_path {
            // Remove existing mount point if it exists
            if config.mount_point.exists() {
                if config.mount_point.is_symlink() {
                    std::fs::remove_file(&config.mount_point)?;
                } else if config.mount_point.is_dir() {
                    // Only remove if empty
                    if std::fs::read_dir(&config.mount_point)?.next().is_none() {
                        std::fs::remove_dir(&config.mount_point)?;
                    } else {
                        anyhow::bail!(
                            "Mount point {} exists and is not empty",
                            config.mount_point.display()
                        );
                    }
                }
            }

            // Create parent directories if needed
            if let Some(parent) = config.mount_point.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Create symlink
            std::os::unix::fs::symlink(&shm_path, &config.mount_point)?;
        }

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn destroy_macos(mount_point: &std::path::Path) -> Result<()> {
        use std::process::Command;

        let output = Command::new("hdiutil")
            .args(["detach", &mount_point.to_string_lossy()])
            .output()?;

        if !output.status.success() {
            anyhow::bail!(
                "Failed to detach ramdisk: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn destroy_linux(mount_point: &std::path::Path) -> Result<()> {
        // Check if mount_point is a symlink to /dev/shm
        if mount_point.is_symlink() {
            let target = std::fs::read_link(mount_point)?;

            // Remove the symlink
            std::fs::remove_file(mount_point)?;

            // Remove the actual directory in /dev/shm if it exists
            if target.starts_with("/dev/shm") && target.exists() {
                std::fs::remove_dir_all(&target)?;
            }
        } else if mount_point.starts_with("/dev/shm") {
            // Direct /dev/shm path
            if mount_point.exists() {
                std::fs::remove_dir_all(mount_point)?;
            }
        } else {
            anyhow::bail!(
                "Mount point {} is not a valid shm ramdisk",
                mount_point.display()
            );
        }

        Ok(())
    }
}
