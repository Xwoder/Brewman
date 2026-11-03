use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

use crate::brew::Msg;
use crate::model::{self, Package};

pub(crate) fn load_packages() -> Result<Vec<Package>, String> {
    let out = exec(&["info".into(), "--json=v2".into(), "--installed".into()])?;
    model::parse_installed_info(&out)
}

pub(crate) fn load_outdated() -> Result<Vec<(String, String)>, String> {
    // --greedy：把 auto_updates 的 cask、:latest / HEAD 安装的包也纳入过时检测
    let out = exec(&["outdated".into(), "--json=v2".into(), "--greedy".into()])?;
    model::parse_outdated(&out)
}

pub(crate) fn to_pair(result: Result<String, String>) -> (bool, String) {
    match result {
        Ok(o) => (true, o),
        Err(e) => (false, e),
    }
}

/// 执行 brew 命令，成功返回 Ok(stdout)，失败返回 Err(错误信息)
pub(crate) fn exec(args: &[String]) -> Result<String, String> {
    let output = Command::new("brew").args(args).output().map_err(|e| {
        format!("Failed to run brew: {e} (make sure Homebrew is installed and on PATH)")
    })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).into_owned();
        if err.trim().is_empty() {
            Err(format!(
                "brew {} failed (exit code {:?})",
                args.join(" "),
                output.status.code()
            ))
        } else {
            Err(err)
        }
    }
}

/// 流式执行 brew 命令：stdout/stderr 逐行通过 Msg::Progress 实时发回 UI，
/// 同时累积完整输出，命令结束后返回（成功 Ok(stdout)，失败 Err(stderr)）
pub(crate) fn exec_streamed(args: &[String], tx: &Sender<Msg>) -> Result<String, String> {
    let mut child = Command::new("brew")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!("Failed to run brew: {e} (make sure Homebrew is installed and on PATH)")
        })?;

    let child_stdout = child.stdout.take().expect("stdout is piped");
    let child_stderr = child.stderr.take().expect("stderr is piped");

    // 两个线程分别读 stdout / stderr，避免管道写满导致子进程阻塞
    let tx_out = tx.clone();
    let tx_err = tx.clone();
    let out_handle = std::thread::spawn(move || read_stream(child_stdout, &tx_out));
    let err_handle = std::thread::spawn(move || read_stream(child_stderr, &tx_err));

    let status = child
        .wait()
        .map_err(|e| format!("Failed to wait for brew: {e}"))?;
    let stdout_text = out_handle.join().unwrap_or_default();
    let stderr_text = err_handle.join().unwrap_or_default();

    if status.success() {
        Ok(stdout_text)
    } else {
        let err = stderr_text.trim().to_string();
        if err.is_empty() {
            Err(format!(
                "brew {} failed (exit code {:?})",
                args.join(" "),
                status.code()
            ))
        } else {
            Err(err)
        }
    }
}

/// 读取一个输出流，每行通过 Msg::Progress 发送，同时累积返回完整文本
pub(crate) fn read_stream<R: std::io::Read>(stream: R, tx: &Sender<Msg>) -> String {
    let reader = BufReader::new(stream);
    let mut buf = String::new();
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let _ = tx.send(Msg::Progress(line.clone()));
        buf.push_str(&line);
        buf.push('\n');
    }
    buf
}
