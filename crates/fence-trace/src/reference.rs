//! Client for the C# parity reference (tools/reference-cs), built from the untouched 1.1.1 sources.
//!
//! The reference is a test tool, never shipped. Build it through low_run (docs/rust2/CONVENTIONS.md), then
//! point `MF_REFERENCE_EXE` at `MouseFence.Reference.exe`. Reference tests are `#[ignore]`d so a plain
//! `cargo test` works without .NET; the parity runs use `-- --ignored` and FAIL when the variable is unset.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// Environment variable naming the reference executable.
pub const REFERENCE_ENV: &str = "MF_REFERENCE_EXE";

pub struct Reference {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Reference {
    /// Spawn from `MF_REFERENCE_EXE`. Errors (never skips) when it is unset or missing.
    pub fn from_env() -> Result<Self, String> {
        let exe = std::env::var_os(REFERENCE_ENV)
            .ok_or_else(|| format!("{REFERENCE_ENV} is not set; build tools/reference-cs first"))?;
        Self::spawn(&PathBuf::from(exe))
    }

    pub fn spawn(exe: &Path) -> Result<Self, String> {
        if !exe.is_file() {
            return Err(format!("reference not found: {}", exe.display()));
        }
        let mut cmd = Command::new(exe);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| format!("spawn {}: {e}", exe.display()))?;
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("no stdout")?);
        Ok(Self { child, stdin, stdout })
    }

    /// Send one request object, read one response line. A response with `"error"` is returned as Err.
    pub fn call(&mut self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        let line = serde_json::to_string(request).map_err(|e| e.to_string())?;
        self.stdin.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        self.stdin.write_all(b"\n").map_err(|e| e.to_string())?;
        self.stdin.flush().map_err(|e| e.to_string())?;
        let mut answer = String::new();
        let n = self.stdout.read_line(&mut answer).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("reference closed its output".to_string());
        }
        let value: serde_json::Value =
            serde_json::from_str(answer.trim_end()).map_err(|e| format!("bad reference line: {e}: {answer}"))?;
        if let Some(err) = value.get("error") {
            return Err(format!("reference error: {err}"));
        }
        Ok(value)
    }

    /// Serialize any request type and call.
    pub fn call_typed<T: serde::Serialize>(&mut self, request: &T) -> Result<serde_json::Value, String> {
        let v = serde_json::to_value(request).map_err(|e| e.to_string())?;
        self.call(&v)
    }
}

impl Drop for Reference {
    fn drop(&mut self) {
        // Closing stdin ends the reference's read loop; wait so no orphan process stays behind.
        let _ = self.stdin.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
