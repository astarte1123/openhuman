use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::RwLock;
use std::time::Duration;

use rand::RngCore;

static RPC_TOKEN: once_cell::sync::Lazy<RwLock<Option<String>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(None));

pub struct CoreProcessHandle {
    port: u16,
    rpc_token: String,
}

impl CoreProcessHandle {
    pub fn new(port: u16) -> Self {
        let mut buf = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut buf);
        let token = hex::encode(buf);
        Self { port, rpc_token: token }
    }

    pub fn rpc_token(&self) -> &str {
        &self.rpc_token
    }

    fn is_port_open(&self) -> bool {
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", self.port)
                .parse()
                .expect("valid socket addr"),
            Duration::from_millis(300),
        )
        .is_ok()
    }

    /// Try to connect to an already-running openhuman-core on the configured port.
    /// If found, reads the RPC token from the standard workspace location.
    pub fn try_attach(&self) -> Option<String> {
        if !self.is_port_open() {
            return None;
        }
        let token = read_core_token();
        *RPC_TOKEN.write().unwrap() = token.clone();
        token
    }

    /// Ensure the core is running. Attempts:
    ///   1. Attach to an already-running instance.
    ///   2. Launch `openhuman-core serve` as a subprocess.
    pub fn ensure_running(&self) -> Result<String, String> {
        if let Some(token) = self.try_attach() {
            eprintln!("[core_process] attached to running core on port {}", self.port);
            return Ok(token);
        }

        let binary = find_openhuman_core_binary()
            .ok_or_else(|| {
                format!(
                    "openhuman-core binary not found and no core is listening on port {}. \
                     Start `openhuman-core serve` manually or build it first.",
                    self.port
                )
            })?;

        eprintln!("[core_process] launching {:?}", binary);
        std::process::Command::new(&binary)
            .arg("serve")
            .spawn()
            .map_err(|e| format!("failed to spawn openhuman-core: {}", e))?;

        // Wait up to 10 s for the port.
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(100));
            if self.is_port_open() {
                let token = read_core_token();
                *RPC_TOKEN.write().unwrap() = token.clone();
                eprintln!("[core_process] core ready on port {}", self.port);
                return Ok(token.unwrap_or_else(|| "no-token".into()));
            }
        }
        Err(format!("openhuman-core did not become ready on port {} in time", self.port))
    }
}

/// Read the RPC bearer token from the standard openhuman workspace locations.
fn read_core_token() -> Option<String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let home = std::path::Path::new(&home);
    for subdir in &[".openhuman-staging", ".openhuman"] {
        let path = home.join(subdir).join("core.token");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            let token = raw.trim().to_string();
            if !token.is_empty() {
                return Some(token);
            }
        }
    }
    None
}

/// Search for the `openhuman-core` binary next to the Tauri executable or on PATH.
fn find_openhuman_core_binary() -> Option<PathBuf> {
    // 1. Sibling of our own executable (packaged in the app bundle).
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent()?;
        for name in &["openhuman-core.exe", "openhuman-core"] {
            let candidate = dir.join(name);
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }

    // 2. PATH lookup.
    let names: &[&str] = if cfg!(windows) {
        &["openhuman-core.exe"]
    } else {
        &["openhuman-core"]
    };
    for name in names {
        if let Some(found) = std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths).find_map(|dir| {
                let candidate = dir.join(name);
                candidate.exists().then_some(candidate)
            })
        }) {
            return Some(found);
        }
    }

    None
}

pub fn current_rpc_token() -> Option<String> {
    RPC_TOKEN.read().unwrap().clone()
}
