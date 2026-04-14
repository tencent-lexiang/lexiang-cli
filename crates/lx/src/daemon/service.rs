use crate::daemon::pidfile::PidFile;
use crate::vfs::VfsManager;
use anyhow::Result;
use std::path::PathBuf;

const DAEMON_NAME: &str = "lexiang-daemon";

pub struct DaemonManager {
    pidfile: PidFile,
    vfs_manager: VfsManager,
}

impl DaemonManager {
    pub fn new(mount_point: Option<PathBuf>, size_mb: Option<u64>) -> Self {
        Self {
            pidfile: PidFile::new(DAEMON_NAME),
            vfs_manager: VfsManager::new(None, size_mb, mount_point),
        }
    }

    /// Start daemon
    pub fn start(&self) -> Result<()> {
        // Check if already running
        if self.pidfile.is_running() {
            let pid = self.pidfile.read()?.unwrap();
            anyhow::bail!("守护进程已在运行 (PID: {})", pid);
        }

        println!("启动守护进程...");

        // Create PID file
        self.pidfile.create()?;

        // Start virtual filesystem
        self.vfs_manager.start()?;

        // Setup signal handlers
        self.setup_signal_handlers()?;

        println!("守护进程已启动 (PID: {})", std::process::id());
        println!("虚拟文件系统挂载点: {:?}", self.vfs_manager.mount_point());

        // Keep running
        self.run()?;

        Ok(())
    }

    /// Stop daemon
    pub fn stop(&self) -> Result<()> {
        let Some(pid) = self.pidfile.read()? else {
            println!("守护进程未运行");
            return Ok(());
        };

        println!("停止守护进程 (PID: {})...", pid);

        // Send SIGTERM
        #[cfg(unix)]
        {
            use libc::kill;
            // SAFETY: kill(pid, signal) is safe - we're sending a signal to a known process
            #[allow(unsafe_code)]
            unsafe {
                if kill(pid as i32, signal_hook::consts::SIGTERM) != 0 {
                    anyhow::bail!("无法停止进程 {}", pid);
                }
            }
        }

        #[cfg(not(unix))]
        {
            let _ = pid;
            anyhow::bail!("守护进程管理在 Windows 平台暂不支持");
        }

        // Wait for process to stop
        #[cfg(unix)]
        {
            let mut retries = 10;
            while self.pidfile.is_running() && retries > 0 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                retries -= 1;
            }

            // Force kill if still running
            if self.pidfile.is_running() {
                use libc::kill;
                // SAFETY: kill(pid, SIGKILL) is safe - we're forcefully terminating a process
                #[allow(unsafe_code)]
                unsafe {
                    kill(pid as i32, signal_hook::consts::SIGKILL);
                }
            }
        }

        // Clean up PID file
        self.pidfile.remove()?;

        println!("守护进程已停止");

        Ok(())
    }

    /// Get daemon status
    pub fn status(&self) -> Result<DaemonStatus> {
        let running = self.pidfile.is_running();
        let pid = if running { self.pidfile.read()? } else { None };
        let vfs_status = if running {
            Some(self.vfs_manager.status()?)
        } else {
            None
        };

        Ok(DaemonStatus {
            running,
            pid,
            vfs_status,
        })
    }

    fn setup_signal_handlers(&self) -> Result<()> {
        #[cfg(unix)]
        {
            use signal_hook::consts::{SIGINT, SIGTERM};
            use signal_hook::iterator::Signals;

            let mut signals = Signals::new([SIGTERM, SIGINT])?;

            let mount_point = self.vfs_manager.mount_point().clone();

            std::thread::spawn(move || {
                // Wait for signal
                let _sig = signals.wait();

                // Cleanup on signal
                println!("\n收到停止信号，正在清理...");

                // Stop VFS
                let vfs = VfsManager::new(None, None, Some(mount_point.clone()));
                let _ = vfs.stop();

                // Remove PID file
                let pidfile = PidFile::new(DAEMON_NAME);
                let _ = pidfile.remove();

                std::process::exit(0);
            });
        }

        Ok(())
    }

    fn run(&self) -> Result<()> {
        // Main daemon loop
        loop {
            // Keep the process alive
            std::thread::sleep(std::time::Duration::from_secs(60));

            // Health check
            if !self.vfs_manager.status()?.mounted {
                eprintln!("虚拟文件系统已丢失，尝试重新挂载...");
                self.vfs_manager.start()?;
            }
        }
    }
}

#[derive(Debug)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub vfs_status: Option<crate::vfs::VfsStatus>,
}
