//! 命令面板中可用的命令注册表。
//!
//! 每条命令包含名称、默认快捷键、作用说明以及其对应的 [`CommandAction`]。
//! 面板按命令名称（及快捷键、说明）做不区分大小写的子串匹配，供用户在输入栏中模糊检索。

use crate::app::state::CommandAction;

/// 命令面板中的一条命令定义
pub struct CommandDef {
    /// 命令名称（用于搜索匹配与展示）
    pub name: &'static str,
    /// 触发该命令的默认快捷键（仅用于展示）
    pub key: &'static str,
    /// 命令作用说明
    pub description: &'static str,
    /// 选中后执行的动作
    pub action: CommandAction,
}

impl CommandDef {
    /// 命令是否匹配查询：空查询匹配全部，否则对名称 / 快捷键 / 说明做不区分大小写的子串匹配
    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_lowercase();
        self.name.to_lowercase().contains(&q)
            || self.key.to_lowercase().contains(&q)
            || self.description.to_lowercase().contains(&q)
    }
}

/// 命令面板中列出的全部命令（顺序即默认展示顺序）
pub const COMMANDS: &[CommandDef] = &[
    CommandDef {
        name: "Upgrade",
        key: "u",
        description: "Upgrade the current package, or all selected packages",
        action: CommandAction::Upgrade,
    },
    CommandDef {
        name: "Upgrade All",
        key: "a",
        description: "Upgrade all outdated packages",
        action: CommandAction::UpgradeAll,
    },
    CommandDef {
        name: "Uninstall",
        key: "x",
        description: "Uninstall the current package",
        action: CommandAction::Uninstall,
    },
    CommandDef {
        name: "Update",
        key: "r",
        description: "Update Homebrew sources (brew update)",
        action: CommandAction::Update,
    },
    CommandDef {
        name: "Clean",
        key: "c",
        description: "Choose a cleanup mode and run brew cleanup / brew cleanup --scrub",
        action: CommandAction::Clean,
    },
    CommandDef {
        name: "Mirror",
        key: "m",
        description: "Select mirror source for the next upgrade",
        action: CommandAction::Mirror,
    },
    CommandDef {
        name: "Reload",
        key: "l",
        description: "Reload the package list",
        action: CommandAction::Reload,
    },
    CommandDef {
        name: "Toggle Outdated",
        key: "o",
        description: "Toggle the 'Outdated only' filter",
        action: CommandAction::ToggleOutdated,
    },
    CommandDef {
        name: "Select",
        key: "Space",
        description: "Select / deselect the current package for batch upgrade",
        action: CommandAction::Select,
    },
    CommandDef {
        name: "View All",
        key: "1",
        description: "Show all packages (formulae + casks)",
        action: CommandAction::TabAll,
    },
    CommandDef {
        name: "View Formulae",
        key: "2",
        description: "Show formulae only",
        action: CommandAction::TabFormula,
    },
    CommandDef {
        name: "View Cask",
        key: "3",
        description: "Show casks only",
        action: CommandAction::TabCask,
    },
    CommandDef {
        name: "Quit",
        key: "q",
        description: "Quit Brewman",
        action: CommandAction::Quit,
    },
];

/// 返回匹配查询的命令列表（空查询返回全部，保持 `COMMANDS` 定义顺序）
pub fn matching(query: &str) -> Vec<&'static CommandDef> {
    COMMANDS.iter().filter(|c| c.matches(query)).collect()
}
