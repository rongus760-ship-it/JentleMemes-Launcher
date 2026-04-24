use crate::error::{Error, Result};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn spawn(
    java_path: &str,
    args: &[String],
    game_dir: &Path,
    instance_id: &str,
) -> Result<std::process::Child> {
    let mut cmd = Command::new(java_path);
    cmd.args(args);
    cmd.current_dir(game_dir);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000u32);
    }

    cmd.spawn()
        .map_err(|e| Error::Custom(format!("Не удалось запустить Java: {e}")))
}
