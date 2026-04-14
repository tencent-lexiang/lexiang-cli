use crate::vfs::ramdisk::{Ramdisk, RamdiskConfig};
use anyhow::Result;
use std::path::PathBuf;

pub struct VfsManager {
    config: RamdiskConfig,
}

impl VfsManager {
    pub fn new(name: Option<String>, size_mb: Option<u64>, mount_point: Option<PathBuf>) -> Self {
        let mut config = RamdiskConfig::default();
        if let Some(n) = name {
            config.name = n;
        }
        if let Some(s) = size_mb {
            config.size_mb = s;
        }
        if let Some(m) = mount_point {
            config.mount_point = m;
        }
        Self { config }
    }

    /// Start virtual filesystem
    pub fn start(&self) -> Result<()> {
        if Ramdisk::is_mounted(&self.config.mount_point) {
            println!("虚拟文件系统已挂载: {}", self.config.mount_point.display());
            return Ok(());
        }

        println!("创建虚拟文件系统 ({}MB)...", self.config.size_mb);
        Ramdisk::create(&self.config)?;
        println!("虚拟文件系统已创建: {}", self.config.mount_point.display());

        Ok(())
    }

    /// Stop virtual filesystem
    pub fn stop(&self) -> Result<()> {
        if !Ramdisk::is_mounted(&self.config.mount_point) {
            println!("虚拟文件系统未挂载");
            return Ok(());
        }

        println!("销毁虚拟文件系统...");
        Ramdisk::destroy(&self.config.mount_point)?;
        println!("虚拟文件系统已销毁");

        Ok(())
    }

    /// Check status
    pub fn status(&self) -> Result<VfsStatus> {
        let mounted = Ramdisk::is_mounted(&self.config.mount_point);

        let usage = if mounted {
            self.get_usage()?
        } else {
            VfsUsage::default()
        };

        Ok(VfsStatus {
            mounted,
            mount_point: self.config.mount_point.clone(),
            size_mb: self.config.size_mb,
            usage,
        })
    }

    fn get_usage(&self) -> Result<VfsUsage> {
        #[cfg(unix)]
        {
            let _ = &self.config.mount_point; // Use to suppress warning
                                              // Note: This is a simplified version. Real implementation would use statvfs
            Ok(VfsUsage {
                used_mb: 0,
                available_mb: self.config.size_mb,
                file_count: 0,
            })
        }

        #[cfg(not(unix))]
        {
            Ok(VfsUsage::default())
        }
    }

    /// Get mount point
    pub fn mount_point(&self) -> &PathBuf {
        &self.config.mount_point
    }
}

#[derive(Debug)]
pub struct VfsStatus {
    pub mounted: bool,
    pub mount_point: PathBuf,
    pub size_mb: u64,
    #[allow(dead_code)]
    pub usage: VfsUsage,
}

#[derive(Debug, Default)]
pub struct VfsUsage {
    #[allow(dead_code)]
    pub used_mb: u64,
    #[allow(dead_code)]
    pub available_mb: u64,
    #[allow(dead_code)]
    pub file_count: u64,
}
