use std::process::Command;
use std::sync::mpsc::{Receiver, Sender};

use crate::model::{self, Kind, Package};

/// UI 线程发给后台工作线程的任务
pub enum Job {
    /// 加载已安装包列表（并检查过时信息）
    Load,
    /// 更新 Homebrew 软件源（brew update），完成后重新加载
    Update,
    /// 升级单个包
    Upgrade { name: String, kind: Kind },
    /// 升级所有过时包
    UpgradeAll,
    /// 卸载单个包
    Uninstall { name: String, kind: Kind },
}

/// 后台工作线程发给 UI 线程的消息
pub enum Msg {
    /// 命令开始执行
    Loading,
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
                let (label, ok, output) = run("Update software sources (brew update)", &["update"]);
                let _ = msg_tx.send(Msg::Done { label, ok, output });
                let _ = msg_tx.send(Msg::Reload);
            }
            Job::Upgrade { name, kind } => {
                let _ = msg_tx.send(Msg::Loading);
                let args = match kind {
                    Kind::Formula => vec!["upgrade".to_string(), name.clone()],
                    Kind::Cask => vec!["upgrade".to_string(), "--cask".to_string(), name.clone()],
                };
                let label = format!("Upgrade {} (brew {})", name, args.join(" "));
                let (ok, output) = to_pair(exec(&args));
                let _ = msg_tx.send(Msg::Done {
                    label,
                    ok,
                    output,
                });
                let _ = msg_tx.send(Msg::Reload);
            }
            Job::UpgradeAll => {
                let _ = msg_tx.send(Msg::Loading);
                let (label, ok, output) = run("Upgrade all outdated packages (brew upgrade)", &["upgrade"]);
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
                let (ok, output) = to_pair(exec(&args));
                let _ = msg_tx.send(Msg::Done {
                    label,
                    ok,
                    output,
                });
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
    let out = exec(&["outdated".into(), "--json=v2".into()])?;
    model::parse_outdated(&out)
}

/// 执行 brew 命令，返回 (label, ok, output)
fn run(label: &str, args: &[&str]) -> (String, bool, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (ok, output) = to_pair(exec(&args));
    (label.to_string(), ok, output)
}

fn to_pair(result: Result<String, String>) -> (bool, String) {
    match result {
        Ok(o) => (true, o),
        Err(e) => (false, e),
    }
}

/// 执行 brew 命令，成功返回 Ok(stdout)，失败返回 Err(错误信息)
fn exec(args: &[String]) -> Result<String, String> {
    let output = Command::new("brew")
        .args(args)
        .output()
        .map_err(|e| format!("Failed to run brew: {e} (make sure Homebrew is installed and on PATH)"))?;

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
