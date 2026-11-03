use std::sync::mpsc::{Receiver, Sender};

use crate::brew::commands::{exec_streamed, load_outdated, load_packages, to_pair};
use crate::brew::{Job, Msg};

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
            Job::Clean => {
                let _ = msg_tx.send(Msg::Loading);
                let label = "Clean up (brew cleanup)".to_string();
                let (ok, output) = to_pair(exec_streamed(&["cleanup".to_string()], &msg_tx));
                let _ = msg_tx.send(Msg::Done { label, ok, output });
                let _ = msg_tx.send(Msg::Reload);
            }
            Job::Uninstall { name, kind } => {
                let _ = msg_tx.send(Msg::Loading);
                let args = match kind {
                    crate::model::Kind::Formula => vec!["uninstall".to_string(), name.clone()],
                    crate::model::Kind::Cask => {
                        vec!["uninstall".to_string(), "--cask".to_string(), name.clone()]
                    }
                };
                let label = format!("Uninstall {} (brew {})", name, args.join(" "));
                let (ok, output) = to_pair(exec_streamed(&args, &msg_tx));
                let _ = msg_tx.send(Msg::Done { label, ok, output });
                let _ = msg_tx.send(Msg::Reload);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::worker;
    use crate::brew::{Job, Msg};
    use crate::model::Kind;
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
