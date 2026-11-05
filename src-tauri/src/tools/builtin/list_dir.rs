// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入路径类型
use std::path::{Component, Path, PathBuf};
// 引入工具说明书相关的数据结构
use crate::protocol::{ToolDefinition, ToolFunction};
// 引入沙箱：解析路径，以及「path 可以省略」的参数助手
use crate::tools::sandbox::{arg_path_or, resolve_inside};
// 引入工具统一接口
use crate::tools::Tool;

// 这些目录名对模型通常是噪声，而且可能非常大。只跳过目录，同名文件仍然列出。
const IGNORED_DIR_NAMES: &[&str] = &[".git", "node_modules", "target"];
// 一层目录也可能有上万个条目，封顶后在结果里说明总数
const MAX_LIST_ENTRIES: usize = 200;

// 定义内置工具：列出工作区内某个目录的直接子项
pub struct ListDir {
    // 工作区根。注册表构造工具时塞进来
    root: PathBuf,
}

impl ListDir {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

// 列目录的纯结果，render 负责变成给模型看的文本
struct DirListing {
    entries: Vec<String>,
    skipped: Vec<String>,
    total_entries: usize,
    truncated: bool,
}

impl DirListing {
    fn render(&self) -> String {
        let mut lines = Vec::new();
        if !self.skipped.is_empty() {
            lines.push(format!("（已跳过目录：{}）", self.skipped.join(", ")));
        }
        if self.truncated {
            lines.push(format!(
                "（共 {} 项，只显示前 {} 项）",
                self.total_entries,
                self.entries.len()
            ));
        }
        if self.entries.is_empty() {
            // 跳过了子目录但没有别的条目时，不要再写「空目录」，跳过说明已经够了
            if self.skipped.is_empty() && !self.truncated {
                lines.push("（空目录）".to_string());
            }
        } else {
            lines.extend(self.entries.iter().cloned());
        }
        lines.join("\n")
    }
}

// 为它实现 Tool 接口
#[async_trait]
impl Tool for ListDir {
    fn name(&self) -> &'static str {
        "list_dir"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function".to_string(),
            function: ToolFunction {
                name: self.name().to_string(),
                description: "列出工作区内一个目录的直接子项，不递归。目录名以 / 结尾，符号链接以 @ 结尾。会跳过名为 node_modules、target、.git 的子目录；路径里任何一段是这些名字也会拒绝。path 省略或为空时列出工作区根目录。path 不能逃出工作区。".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "相对工作区根目录的目录路径，省略时列出根目录"
                        }
                    },
                    "required": []
                }),
            },
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let path = arg_path_or(&args, "path", ".")?;
        let root = resolve_inside(&self.root, ".")?;
        let resolved = resolve_inside(&self.root, &path)?;
        ensure_listable(&root, &path, &resolved)?;
        let listing = list_directory(&resolved, MAX_LIST_ENTRIES)?;
        Ok(listing.render())
    }
}

// 路径里任何一段是 node_modules / target / .git 都直接拒绝，包括它们的子目录。
// `node_modules/../src` 把 `..` 抵消之后不再经过这些名字，仍然允许。
// 工作区根自己碰巧叫这些名字时，列 "." 没有相对分量，所以仍然允许。
fn ensure_listable(root: &Path, user_path: &str, resolved: &Path) -> anyhow::Result<()> {
    if let Some(name) = ignored_dir_in_path(user_path) {
        anyhow::bail!("目录 {name} 被忽略，不会列出");
    }
    // 符号链接 canonicalize 之后可能落进被忽略的目录，再按真实相对路径查一次
    if let Ok(relative) = resolved.strip_prefix(root) {
        for component in relative.components() {
            if let Component::Normal(segment) = component
                && let Some(name) = segment.to_str()
                && is_ignored_dir(name)
            {
                anyhow::bail!("目录 {name} 被忽略，不会列出");
            }
        }
    }
    if !resolved.is_dir() {
        anyhow::bail!("不是目录: {user_path}");
    }
    Ok(())
}

// 先抵消 `.` 和 `..`，再看剩下的每一段。
fn ignored_dir_in_path(user_path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for component in Path::new(user_path.trim()).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop();
            }
            Component::Normal(segment) => {
                parts.push(segment.to_string_lossy().into_owned());
            }
            Component::Prefix(_) | Component::RootDir => {}
        }
    }
    parts.into_iter().find(|name| is_ignored_dir(name))
}

fn is_ignored_dir(name: &str) -> bool {
    IGNORED_DIR_NAMES.contains(&name)
}

// 只列一层。max_entries 单独传进来，测试可以用很小的上限，不必造 200 个文件。
fn list_directory(dir: &Path, max_entries: usize) -> anyhow::Result<DirListing> {
    if max_entries == 0 {
        anyhow::bail!("列表上限必须大于 0");
    }
    let mut skipped = Vec::new();
    let mut found = Vec::new();
    let read = std::fs::read_dir(dir)
        .map_err(|err| anyhow::anyhow!("列出目录失败 {}: {err}", dir.display()))?;
    for entry in read {
        let entry =
            entry.map_err(|err| anyhow::anyhow!("列出目录失败 {}: {err}", dir.display()))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // file_type 不跟随符号链接，所以「指向目录的链接」不会被当成目录跳过或递归
        let file_type = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("读取条目类型失败 {name}: {err}"))?;
        if file_type.is_dir() && is_ignored_dir(&name) {
            skipped.push(name);
            continue;
        }
        let label = if file_type.is_symlink() {
            format!("{name}@")
        } else if file_type.is_dir() {
            format!("{name}/")
        } else {
            name.clone()
        };
        found.push((name, label));
    }
    // 按原始名字排，而不是按带 `/` 的展示名排，避免斜杠改变顺序
    found.sort_by(|a, b| a.0.cmp(&b.0));
    skipped.sort();
    let total_entries = found.len();
    let truncated = total_entries > max_entries;
    if truncated {
        found.truncate(max_entries);
    }
    Ok(DirListing {
        entries: found.into_iter().map(|(_, label)| label).collect(),
        skipped,
        total_entries,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::sandbox::TestWorkspace;

    #[tokio::test]
    async fn lists_one_level_and_skips_junk_dirs() {
        let ws = TestWorkspace::new();
        ws.write("a.txt", "a");
        ws.write("b.txt", "b");
        std::fs::create_dir(ws.path().join("keep")).unwrap();
        for name in ["node_modules", "target", ".git"] {
            let dir = ws.path().join(name);
            std::fs::create_dir(&dir).unwrap();
            std::fs::write(dir.join("noise"), "x").unwrap();
        }
        let tool = ListDir::new(ws.path());
        let out = tool.execute(serde_json::json!({})).await.unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.contains(&"a.txt"), "{out}");
        assert!(lines.contains(&"b.txt"), "{out}");
        assert!(lines.contains(&"keep/"), "{out}");
        assert!(
            !lines.contains(&"node_modules") && !lines.contains(&"node_modules/"),
            "{out}"
        );
        assert!(
            !lines.contains(&"target") && !lines.contains(&"target/"),
            "{out}"
        );
        assert!(
            !lines.contains(&".git") && !lines.contains(&".git/"),
            "{out}"
        );
        assert!(
            lines.iter().any(|line| line.contains("已跳过目录")),
            "{out}"
        );
        assert!(out.contains(".git") && out.contains("node_modules") && out.contains("target"));
        // 不递归：keep 里面的文件不该出现在根列表里
        ws.write("keep/hidden.txt", "h");
        let again = tool
            .execute(serde_json::json!({"path": "."}))
            .await
            .unwrap();
        assert!(!again.lines().any(|line| line.contains("hidden.txt")));
    }

    #[tokio::test]
    async fn file_named_like_junk_is_listed_but_junk_dir_is_refused() {
        let ws = TestWorkspace::new();
        ws.write("target", "not-a-directory");
        std::fs::create_dir(ws.path().join("node_modules")).unwrap();
        let tool = ListDir::new(ws.path());
        let out = tool.execute(serde_json::json!({"path": ""})).await.unwrap();
        assert!(out.lines().any(|line| line == "target"), "{out}");
        let err = tool
            .execute(serde_json::json!({"path": "node_modules"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("忽略"), "{err}");
        std::fs::create_dir_all(ws.path().join("node_modules/pkg")).unwrap();
        let err = tool
            .execute(serde_json::json!({"path": "node_modules/pkg"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("忽略"), "{err}");
        std::fs::create_dir(ws.path().join("keep")).unwrap();
        let nested = tool
            .execute(serde_json::json!({"path": "node_modules/../keep"}))
            .await
            .unwrap();
        assert!(
            nested.contains("空目录") || nested.contains("keep"),
            "{nested}"
        );
        ws.write("f.txt", "x");
        let err = tool
            .execute(serde_json::json!({"path": "f.txt"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("不是目录"), "{err}");
        let err = tool
            .execute(serde_json::json!({"path": ".."}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        let empty = TestWorkspace::new();
        let out = ListDir::new(empty.path())
            .execute(serde_json::json!({}))
            .await
            .unwrap();
        assert_eq!(out, "（空目录）");
    }

    #[test]
    fn truncates_long_listings() {
        let ws = TestWorkspace::new();
        for name in ["a", "b", "c", "d"] {
            ws.write(name, "x");
        }
        let listing = list_directory(ws.path(), 2).unwrap();
        let text = listing.render();
        assert!(text.contains("共 4 项，只显示前 2 项"), "{text}");
        assert!(text.contains('a'));
        assert!(text.contains('b'));
        let lines: Vec<&str> = text.lines().collect();
        assert!(!lines.contains(&"c") && !lines.contains(&"d"), "{text}");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn shows_symlink_suffix() {
        let ws = TestWorkspace::new();
        ws.write("a.txt", "x");
        std::os::unix::fs::symlink("a.txt", ws.path().join("link")).unwrap();
        let out = ListDir::new(ws.path())
            .execute(serde_json::json!({"path": "."}))
            .await
            .unwrap();
        assert!(out.lines().any(|line| line == "link@"), "{out}");
        assert!(out.lines().any(|line| line == "a.txt"), "{out}");
    }
}
