use anyhow::Result;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;

pub struct PidFile {
    path: PathBuf,
}

impl PidFile {
    pub fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("{}.pid", name));
        Self { path }
    }

    pub fn create(&self) -> Result<()> {
        let pid = std::process::id();
        let mut file = fs::File::create(&self.path)?;
        writeln!(file, "{}", pid)?;
        Ok(())
    }

    pub fn read(&self) -> Result<Option<u32>> {
        if !self.path.exists() {
            return Ok(None);
        }

        let Ok(mut file) = fs::File::open(&self.path) else {
            return Ok(None);
        };

        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let pid: u32 = content.trim().parse()?;
        Ok(Some(pid))
    }

    pub fn remove(&self) -> Result<()> {
        if self.path.exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }

    pub fn is_running(&self) -> bool {
        match self.read() {
            Ok(Some(pid)) => {
                // Check if process is running
                #[cfg(unix)]
                {
                    use libc::kill;
                    // SAFETY: kill(pid, 0) is safe - it only checks if process exists
                    #[allow(unsafe_code)]
                    unsafe {
                        kill(pid as i32, 0) == 0
                    }
                }
                #[cfg(not(unix))]
                {
                    false
                }
            }
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &PathBuf {
        &self.path
    }
}
