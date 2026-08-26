use serde::Deserialize;

/// 包的类型：formula（命令行工具）或 cask（桌面应用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Formula,
    Cask,
}

impl Kind {
    pub fn label(&self) -> &'static str {
        match self {
            Kind::Formula => "formula",
            Kind::Cask => "cask",
        }
    }
}

/// 统一表示一个已安装的包
#[derive(Debug, Clone)]
pub struct Package {
    pub kind: Kind,
    /// 用于命令行的标识（formula 名 / cask token）
    pub name: String,
    /// 展示名称（cask 的显示名，formula 同 name）
    pub display_name: String,
    /// 当前安装版本
    pub current_version: Option<String>,
    /// 最新（stable）版本
    pub latest_version: Option<String>,
    /// 候选版本（formula 的 HEAD 版本等）
    pub head_version: Option<String>,
    /// 已安装的全部版本
    pub installed_versions: Vec<String>,
    pub outdated: bool,
    pub pinned: bool,
    pub installed_as_dependency: bool,
    pub installed_on_request: bool,
    pub auto_updates: bool,
    pub dependencies: Vec<String>,
    pub desc: Option<String>,
    pub homepage: Option<String>,
    pub caveats: Option<String>,
}

// ---------- brew info --json=v2 --installed 解析 ----------

#[derive(Deserialize)]
struct InfoV2 {
    #[serde(default)]
    formulae: Vec<FormulaJson>,
    #[serde(default)]
    casks: Vec<CaskJson>,
}

#[derive(Deserialize)]
struct FormulaJson {
    name: String,
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    versions: VersionsJson,
    #[serde(default)]
    installed: Vec<InstalledJson>,
    #[serde(default)]
    pinned: Option<bool>,
    #[serde(default)]
    outdated: Option<bool>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    caveats: Option<String>,
}

#[derive(Deserialize, Default)]
struct VersionsJson {
    #[serde(default)]
    stable: Option<String>,
    #[serde(default)]
    head: Option<String>,
}

#[derive(Deserialize)]
struct InstalledJson {
    version: String,
    #[serde(default)]
    installed_as_dependency: Option<bool>,
    #[serde(default)]
    installed_on_request: Option<bool>,
}

#[derive(Deserialize)]
struct CaskJson {
    token: String,
    #[serde(default)]
    name: Vec<String>,
    #[serde(default)]
    desc: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    installed: Option<String>,
    #[serde(default)]
    outdated: Option<bool>,
    #[serde(default)]
    auto_updates: Option<bool>,
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.filter(|v| !v.is_empty())
}

/// 解析 `brew info --json=v2 --installed` 输出，返回排序后的包列表
pub fn parse_installed_info(json: &str) -> Result<Vec<Package>, String> {
    let info: InfoV2 =
        serde_json::from_str(json).map_err(|e| format!("Failed to parse brew info JSON: {e}"))?;

    let mut packages = Vec::new();

    for f in info.formulae {
        let current = f.installed.first().map(|i| i.version.clone());
        let dep = f
            .installed
            .first()
            .and_then(|i| i.installed_as_dependency)
            .unwrap_or(false);
        let on_request = f
            .installed
            .first()
            .and_then(|i| i.installed_on_request)
            .unwrap_or(false);
        packages.push(Package {
            kind: Kind::Formula,
            name: f.name.clone(),
            display_name: if f.full_name.is_empty() {
                f.name.clone()
            } else {
                f.full_name
            },
            current_version: non_empty(current),
            latest_version: non_empty(f.versions.stable),
            head_version: non_empty(f.versions.head),
            installed_versions: f.installed.iter().map(|i| i.version.clone()).collect(),
            outdated: f.outdated.unwrap_or(false),
            pinned: f.pinned.unwrap_or(false),
            installed_as_dependency: dep,
            installed_on_request: on_request,
            auto_updates: false,
            dependencies: f.dependencies,
            desc: f.desc,
            homepage: f.homepage,
            caveats: f.caveats,
        });
    }

    for c in info.casks {
        let installed_ver = non_empty(c.installed);
        packages.push(Package {
            kind: Kind::Cask,
            name: c.token.clone(),
            display_name: c.name.first().cloned().unwrap_or_else(|| c.token.clone()),
            current_version: installed_ver.clone(),
            latest_version: non_empty(c.version),
            head_version: None,
            installed_versions: installed_ver.into_iter().collect(),
            outdated: c.outdated.unwrap_or(false),
            pinned: false,
            installed_as_dependency: false,
            installed_on_request: false,
            auto_updates: c.auto_updates.unwrap_or(false),
            dependencies: Vec::new(),
            desc: c.desc,
            homepage: c.homepage,
            caveats: None,
        });
    }

    // 过时的排前面，其余按名称排序
    packages.sort_by(|a, b| {
        b.outdated
            .cmp(&a.outdated)
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(packages)
}

// ---------- brew outdated --json=v2 解析 ----------

#[derive(Deserialize)]
struct OutdatedV2 {
    #[serde(default)]
    formulae: Vec<OutdatedFormula>,
    #[serde(default)]
    casks: Vec<OutdatedCask>,
}

#[derive(Deserialize)]
struct OutdatedFormula {
    name: String,
    current_version: String,
}

#[derive(Deserialize)]
struct OutdatedCask {
    name: String,
    current_version: String,
}

/// 解析 `brew outdated --json=v2`，返回 (包名, 最新版本) 列表
pub fn parse_outdated(json: &str) -> Result<Vec<(String, String)>, String> {
    let out: OutdatedV2 = serde_json::from_str(json)
        .map_err(|e| format!("Failed to parse brew outdated JSON: {e}"))?;
    let mut entries = Vec::new();
    for f in out.formulae {
        entries.push((f.name, f.current_version));
    }
    for c in out.casks {
        entries.push((c.name, c.current_version));
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_INFO: &str = r#"{
      "formulae": [
        {
          "name": "wget",
          "full_name": "wget",
          "versions": { "stable": "1.21.4", "head": "HEAD" },
          "installed": [
            { "version": "1.21.3", "installed_as_dependency": false, "installed_on_request": true }
          ],
          "linked_keg": "1.21.3",
          "pinned": false,
          "outdated": true,
          "desc": "Internet file retriever",
          "homepage": "https://www.gnu.org/software/wget/",
          "dependencies": ["openssl@3"],
          "caveats": null
        },
        {
          "name": "nginx",
          "full_name": "nginx",
          "versions": { "stable": "1.25.3", "head": null },
          "installed": [
            { "version": "1.25.3", "installed_as_dependency": true, "installed_on_request": false }
          ],
          "linked_keg": null,
          "pinned": true,
          "outdated": false,
          "desc": null,
          "homepage": null,
          "dependencies": [],
          "caveats": "Please note"
        }
      ],
      "casks": [
        {
          "token": "visual-studio-code",
          "name": ["Visual Studio Code"],
          "desc": "Code editor",
          "homepage": "https://code.visualstudio.com/",
          "version": "1.85.1",
          "installed": "1.84.2",
          "outdated": true,
          "auto_updates": true
        }
      ]
    }"#;

    const SAMPLE_OUTDATED: &str = r#"{
      "formulae": [
        { "name": "wget", "installed_versions": ["1.21.3"], "current_version": "1.21.4" }
      ],
      "casks": [
        { "name": "visual-studio-code", "installed_versions": ["1.84.2"], "current_version": "1.85.1" }
      ]
    }"#;

    #[test]
    fn parses_info() {
        let pkgs = parse_installed_info(SAMPLE_INFO).expect("parse ok");
        assert_eq!(pkgs.len(), 3);

        let wget = pkgs.iter().find(|p| p.name == "wget").expect("wget");
        assert_eq!(wget.kind, Kind::Formula);
        assert_eq!(wget.current_version.as_deref(), Some("1.21.3"));
        assert_eq!(wget.latest_version.as_deref(), Some("1.21.4"));
        assert_eq!(wget.head_version.as_deref(), Some("HEAD"));
        assert!(wget.outdated);

        let nginx = pkgs.iter().find(|p| p.name == "nginx").expect("nginx");
        assert!(nginx.pinned);
        assert!(nginx.installed_as_dependency);
        assert!(!nginx.outdated);

        let vscode = pkgs
            .iter()
            .find(|p| p.name == "visual-studio-code")
            .expect("vscode");
        assert_eq!(vscode.kind, Kind::Cask);
        assert_eq!(vscode.display_name, "Visual Studio Code");
        assert_eq!(vscode.current_version.as_deref(), Some("1.84.2"));
        assert!(vscode.auto_updates);
    }

    #[test]
    fn parses_outdated() {
        let entries = parse_outdated(SAMPLE_OUTDATED).expect("parse ok");
        assert_eq!(entries.len(), 2);
        assert!(entries.contains(&("wget".to_string(), "1.21.4".to_string())));
    }

    #[test]
    fn sort_outdated_first() {
        let pkgs = parse_installed_info(SAMPLE_INFO).expect("parse ok");
        // 两个 outdated 的包排前面（按名称），nginx 排最后
        assert_eq!(pkgs[0].name, "visual-studio-code");
        assert_eq!(pkgs[1].name, "wget");
        assert_eq!(pkgs[2].name, "nginx");
    }
}
