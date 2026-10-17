//! slunk's backend, run beside the app in debug builds until phase 6
//! (design §8). The routes not ported yet still send GraphQL, which Vite
//! proxies to it on port 4000. It serves the open library, and skips its own
//! migrations, since Rust has migrated it already. Release builds never
//! start it.

use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// The port the frontend's Vite proxy expects.
const PORT: u16 = 4000;

/// How long the backend has to start listening.
const START_TIMEOUT: Duration = Duration::from_secs(20);

/// How long it has to close its database once asked to stop.
const STOP_TIMEOUT: Duration = Duration::from_secs(5);

/// The running backend. Dropping it stops it.
pub struct Sidecar {
    child: Child,
}

/// slunk's backend, in `legacy/slunk-be/`.
fn backend() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../legacy/slunk-be")
}

fn address() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], PORT))
}

fn listening() -> bool {
    TcpStream::connect_timeout(&address(), Duration::from_millis(200)).is_ok()
}

impl Sidecar {
    /// Whether the sidecar can start: its port is free, and its dependencies
    /// are installed. Checked before a library opens, so a failure leaves
    /// the open library as it was.
    pub fn check() -> Result<(), String> {
        if !backend().join("node_modules").is_dir() {
            return Err(
                "slunk's backend isn't installed, so the pages that use it can't load. \
                 Run `npm install --prefix legacy/slunk-be`, then try again."
                    .into(),
            );
        }
        if listening() {
            return Err(format!(
                "Something is already listening on port {PORT}, so slunk's backend can't start. \
                 Stop it (it may be slunk itself), then try again."
            ));
        }
        Ok(())
    }

    /// Starts `node src/server.ts` on `db`, and waits until it listens. A
    /// watchdog stops it if the app is gone without stopping it.
    pub fn start(db: &Path) -> Result<Sidecar, String> {
        Self::check()?;
        let watchdog = Path::new(env!("CARGO_MANIFEST_DIR")).join("sidecar-watchdog.cjs");
        let child = Command::new("node")
            .arg("--require")
            .arg(watchdog)
            .arg("src/server.ts")
            .current_dir(backend())
            .env("SLUNK_DB", db)
            .env("SLUNK_SKIP_MIGRATE", "1")
            .env("PORT", PORT.to_string())
            .stdin(Stdio::null())
            .spawn()
            .map_err(|error| format!("Can't start slunk's backend with node: {error}"))?;
        let mut sidecar = Sidecar { child };
        let started = Instant::now();
        while !listening() {
            if let Ok(Some(status)) = sidecar.child.try_wait() {
                return Err(format!(
                    "slunk's backend stopped as it started ({status}). Its output is in the terminal."
                ));
            }
            if started.elapsed() > START_TIMEOUT {
                return Err("slunk's backend didn't start listening in time.".into());
            }
            thread::sleep(Duration::from_millis(100));
        }
        Ok(sidecar)
    }
}

impl Drop for Sidecar {
    /// Asks the backend to stop, which closes its database and so removes
    /// the `-wal` and `-shm` files, then kills it if it hasn't.
    fn drop(&mut self) {
        #[cfg(unix)]
        // SAFETY: kill() only sends a signal, to the process this owns.
        unsafe {
            libc::kill(self.child.id() as libc::pid_t, libc::SIGTERM);
        }
        let asked = Instant::now();
        while asked.elapsed() < STOP_TIMEOUT {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
