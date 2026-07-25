use std::{
    io::{self, Read},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{anyhow, Result};
use command_group::{CommandGroup, GroupChild};

use agentboard_core::registry::ActionContext;

pub(super) enum Run {
    Finished(Output),
    TimedOut(Output),
}

pub(super) fn run(cmd: &str, cwd: Option<&str>, context: &ActionContext<'_>) -> Result<Output> {
    shell_command(cmd, cwd, context)
        .output()
        .map_err(Into::into)
}

pub(super) fn run_until(
    cmd: &str,
    cwd: Option<&str>,
    context: &ActionContext<'_>,
    deadline: Instant,
) -> Result<Run> {
    let mut command = shell_command(cmd, cwd, context);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.group_spawn()?;
    let stdout = child
        .inner()
        .stdout
        .take()
        .ok_or_else(|| anyhow!("healthcheck stdout was not captured"))?;
    let stderr = child
        .inner()
        .stderr
        .take()
        .ok_or_else(|| anyhow!("healthcheck stderr was not captured"))?;
    let stdout_reader = thread::spawn(move || read_output(stdout));
    let stderr_reader = thread::spawn(move || read_output(stderr));

    let (status, timed_out) = loop {
        if let Some(status) = child.try_wait()? {
            // A shell can exit while background descendants still hold its capture
            // pipes. End the remaining probe group before joining the readers.
            let _ = child.kill();
            break (status, false);
        }
        let now = Instant::now();
        if now >= deadline {
            terminate_process_group(&mut child)?;
            break (child.wait()?, true);
        }
        thread::sleep(Duration::from_millis(5).min(deadline - now));
    };
    let output = Output {
        status,
        stdout: join_output_reader(stdout_reader)?,
        stderr: join_output_reader(stderr_reader)?,
    };
    if timed_out {
        Ok(Run::TimedOut(output))
    } else {
        Ok(Run::Finished(output))
    }
}

fn terminate_process_group(child: &mut GroupChild) -> Result<()> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(error) if process_group_is_gone(&error) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn process_group_is_gone(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::InvalidInput | io::ErrorKind::NotFound
    ) || no_such_process(error)
}

#[cfg(unix)]
fn no_such_process(error: &io::Error) -> bool {
    // POSIX ESRCH means the group exited between try_wait and kill.
    error.raw_os_error() == Some(libc::ESRCH)
}

#[cfg(not(unix))]
fn no_such_process(_error: &io::Error) -> bool {
    false
}

fn read_output(mut pipe: impl Read) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    pipe.read_to_end(&mut output)?;
    Ok(output)
}

fn join_output_reader(reader: thread::JoinHandle<io::Result<Vec<u8>>>) -> Result<Vec<u8>> {
    reader
        .join()
        .map_err(|_| anyhow!("healthcheck output reader panicked"))?
        .map_err(Into::into)
}

fn shell_command(cmd: &str, cwd: Option<&str>, context: &ActionContext<'_>) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .env("AGENTBOARD_WORKSPACE_ID", context.workspace_id)
        .env("AGENTBOARD_SOURCE_ID", context.source_id)
        .env("AGENTBOARD_ITEM_ID", &context.item.id);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
}
