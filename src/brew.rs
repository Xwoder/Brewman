use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, Sender};

use crate::model::{self, Kind, Package};

/// UI 线程发给后台工作线程的任务
pub enum Job {
    /// 加载已安装包列表（并检查过时信息）
    Load,
    /// 更新 Homebrew 软件源（brew update），完成后重新加载
    Update,
    /// 卸载单个包
    Uninstall { name: String, kind: Kind },
}

/// 后台工作线程发给 UI 线程的消息
pub enum Msg {
    /// 命令开始执行
    Loading,
    /// 命令的实时输出行（用于展示进度）
    Progress(String),
    /// 已安装包列表
    Packages(Vec<Package>),
    /// 过时包列表：(包名, 最新版本)
    Outdated(Vec<(String, String)>),
    /// 命令执行完毕
    Done {
        label: String,
        ok: bool,
        output: String,
    },
    /// 提示 UI 重新触发 Load
    Reload,
    Error(String),
}

/// 后台工作线程主循环：顺序执行任务（brew 命令不可并发）
pub fn worker(job_rx: Receiver<Job>, msg_tx: Sender<Msg>) {
    while let Ok(job) = job_rx.recv() {
        match job {
            Job::Load => {
                let _ = msg_tx.send(Msg::Loading);
                match load_packages() {
                    Ok(pkgs) => {
                        let _ = msg_tx.send(Msg::Packages(pkgs));
                    }
                    Err(e) => {
                        let _ = msg_tx.send(Msg::Error(e));
                    }
                }
                match load_outdated() {
                    Ok(entries) => {
                        let _ = msg_tx.send(Msg::Outdated(entries));
                    }
                    Err(e) => {
                        let _ = msg_tx.send(Msg::Error(e));
                    }
                }
            }
            Job::Update => {
                let _ = msg_tx.send(Msg::Loading);
                let label = "Update software sources (brew update)".to_string();
                let (ok, output) = to_pair(exec_streamed(&["update".to_string()], &msg_tx));
                let _ = msg_tx.send(Msg::Done { label, ok, output });
                let _ = msg_tx.send(Msg::Reload);
            }
            Job::Uninstall { name, kind } => {
                let _ = msg_tx.send(Msg::Loading);
                let args = match kind {
                    Kind::Formula => vec!["uninstall".to_string(), name.clone()],
                    Kind::Cask => vec!["uninstall".to_string(), "--cask".to_string(), name.clone()],
                };
                let label = format!("Uninstall {} (brew {})", name, args.join(" "));
                let (ok, output) = to_pair(exec_streamed(&args, &msg_tx));
                let _ = msg_tx.send(Msg::Done { label, ok, output });
                let _ = msg_tx.send(Msg::Reload);
            }
        }
    }
}

fn load_packages() -> Result<Vec<Package>, String> {
    let out = exec(&["info".into(), "--json=v2".into(), "--installed".into()])?;
    model::parse_installed_info(&out)
}

fn load_outdated() -> Result<Vec<(String, String)>, String> {
    // --greedy：把 auto_updates 的 cask、:latest / HEAD 安装的包也纳入过时检测
    let out = exec(&["outdated".into(), "--json=v2".into(), "--greedy".into()])?;
    model::parse_outdated(&out)
}

fn to_pair(result: Result<String, String>) -> (bool, String) {
    match result {
        Ok(o) => (true, o),
        Err(e) => (false, e),
    }
}

/// 执行 brew 命令，成功返回 Ok(stdout)，失败返回 Err(错误信息)
fn exec(args: &[String]) -> Result<String, String> {
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
fn exec_streamed(args: &[String], tx: &Sender<Msg>) -> Result<String, String> {
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
fn read_stream<R: std::io::Read>(stream: R, tx: &Sender<Msg>) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    /// 集成测试：真实调用 brew 并解析，验证完整链路
    #[test]
    fn loads_real_brew_packages() {
        let (job_tx, job_rx) = mpsc::channel::<Job>();
        let (msg_tx, msg_rx) = mpsc::channel::<Msg>();
        thread::spawn(move || worker(job_rx, msg_tx));

        job_tx.send(Job::Load).expect("send job");

        let deadline = Instant::now() + Duration::from_secs(90);
        while Instant::now() < deadline {
            match msg_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(Msg::Packages(pkgs)) => {
                    assert!(!pkgs.is_empty(), "at least one package should be installed");
                    assert!(
                        pkgs.iter().any(|p| p.kind == Kind::Formula),
                        "should contain at least one formula"
                    );
                    eprintln!(
                        "Integration test passed: loaded {} packages ({} formulae/{} casks)",
                        pkgs.len(),
                        pkgs.iter().filter(|p| p.kind == Kind::Formula).count(),
                        pkgs.iter().filter(|p| p.kind == Kind::Cask).count()
                    );
                    return;
                }
                Ok(Msg::Error(e)) => {
                    if e.contains("Failed to run brew") {
                        eprintln!("Skipping integration test: brew unavailable - {e}");
                        return;
                    }
                    panic!("Load failed: {e}");
                }
                Ok(_) => {}
                Err(_) => panic!("timed out waiting for message"),
            }
        }
        panic!("no package list received within 90 seconds");
    }
}
