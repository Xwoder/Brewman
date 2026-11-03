use crate::app::{App, ExitCommand, Mirror, PendingAction};
use crate::brew::Job;
use crate::model::Kind;

impl App {
    /// 执行已确认的操作：升级（退出后由用户在终端运行）或卸载（后台执行）。
    pub(crate) fn execute_confirm(&mut self) {
        let Some(confirm) = self.confirm.take() else {
            return;
        };
        match confirm.action {
            PendingAction::Upgrade(i) => {
                if let Some(&idx) = self.filtered.get(i) {
                    let pkg = self.packages[idx].clone();
                    let args = match pkg.kind {
                        Kind::Formula => vec![
                            "upgrade".to_string(),
                            pkg.name,
                            "--yes".to_string(),
                        ],
                        Kind::Cask => vec![
                            "upgrade".to_string(),
                            "--cask".to_string(),
                            pkg.name,
                            "--yes".to_string(),
                        ],
                    };
                    self.exit_command = Some(self.make_exit_command(vec![args]));
                }
            }
            PendingAction::Uninstall(i) => {
                if let Some(&idx) = self.filtered.get(i) {
                    let pkg = self.packages[idx].clone();
                    self.start_job(
                        format!("Uninstalling {}...", pkg.name),
                        Job::Uninstall {
                            name: pkg.name,
                            kind: pkg.kind,
                        },
                    );
                }
            }
            PendingAction::UpgradeAll => {
                self.exit_command = Some(self.make_exit_command(vec![vec![
                    "upgrade".to_string(),
                    "--greedy".to_string(),
                    "--yes".to_string(),
                ]]));
            }
            PendingAction::UpgradeSelected(pkgs) => {
                let invocations = build_upgrade_invocations(&pkgs);
                self.exit_command = Some(self.make_exit_command(invocations));
            }
        }
        // 升级操作不在程序内执行：退出程序，命令交由用户在系统终端运行
        if self.exit_command.is_some() {
            self.should_quit = true;
        }
    }

    /// 由参数数组构造待执行的升级命令：
    /// 不再拼 shell 字符串，而是以「brew 参数数组 + 镜像环境变量」的形式存储，
    /// 最终由 main 直接 `Command::new("brew").args(...).env(...)` 执行。
    fn make_exit_command(&self, invocations: Vec<Vec<String>>) -> ExitCommand {
        let envs = self.mirror.env_pairs();
        let display = {
            let mut lines = Vec::with_capacity(invocations.len());
            for args in &invocations {
                let mut s = String::new();
                for (k, v) in &envs {
                    s.push_str(&format!("{k}=\"{v}\" "));
                }
                s.push_str("brew");
                for a in args {
                    s.push(' ');
                    s.push_str(a);
                }
                lines.push(s);
            }
            lines.join("\n")
        };
        ExitCommand {
            display,
            invocations,
            envs,
        }
    }

    /// 设置升级镜像源并关闭弹窗
    pub(crate) fn set_mirror(&mut self, mirror: Mirror) {
        self.mirror = mirror;
        self.mirror_picker = false;
        self.status = format!("Upgrade mirror set to: {}", mirror.label());
    }
}

/// 构造 brew upgrade 参数数组：formula 与 cask 不能混在一条命令里，需分组输出。
/// 返回值是若干个 brew 调用的参数数组（一行命令对应一组 args）。
fn build_upgrade_invocations(pkgs: &[(String, Kind)]) -> Vec<Vec<String>> {
    let formulae: Vec<&str> = pkgs
        .iter()
        .filter(|(_, k)| *k == Kind::Formula)
        .map(|(n, _)| n.as_str())
        .collect();
    let casks: Vec<&str> = pkgs
        .iter()
        .filter(|(_, k)| *k == Kind::Cask)
        .map(|(n, _)| n.as_str())
        .collect();
    let mut invocations = Vec::new();
    if !formulae.is_empty() {
        let mut args = vec!["upgrade".to_string()];
        args.extend(formulae.iter().map(|s| s.to_string()));
        args.push("--yes".to_string());
        invocations.push(args);
    }
    if !casks.is_empty() {
        let mut args = vec!["upgrade".to_string(), "--cask".to_string()];
        args.extend(casks.iter().map(|s| s.to_string()));
        args.push("--yes".to_string());
        invocations.push(args);
    }
    invocations
}
