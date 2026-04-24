use std::process::Command;

#[cfg(target_os = "linux")]
pub fn set_parent_death_signal(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
pub fn set_parent_death_signal(_cmd: &mut Command) {}
