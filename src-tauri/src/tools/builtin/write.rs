// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入路径类型：工具要记住自己的工作区根
use std::path::PathBuf;
// 引入工具说明书相关的数据结构
use crate::protocol::{ToolDefinition, ToolFunction};
// 引入沙箱：解析路径，并做原子写入
use crate::tools::sandbox::{arg_path, arg_string, atomic_write, resolve_inside};
// 引入工具统一接口
use crate::tools::Tool;

// 定义内置工具：把文本写入工作区内的文件
pub struct WriteFile {
    // 工作区根。注册表构造工具时塞进来
    root: PathBuf,
}

impl WriteFile {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

// 为它实现 Tool 接口
#[async_trait]
impl Tool for WriteFile {
    fn name(&self) -> &'static str {
        "write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function".to_string(),
            function: ToolFunction {
                name: self.name().to_string(),
                description: "把 contents 写入工作区内的文件，已存在则覆盖。写入是原子的：先写同目录临时文件，成功后再改名替换。父目录必须已经存在，不会自动创建。path 不能逃出工作区。".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "相对工作区根目录的文件路径"
                        },
                        "contents": {
                            "type": "string",
                            "description": "要写入的全部文本，空字符串会把文件清空"
                        }
                    },
                    "required": ["path", "contents"]
                }),
            },
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let path = arg_path(&args, "path")?;
        // 空 contents 是合法的：用来清空文件。不要把空白 trim 掉
        let contents = arg_string(&args, "contents")?;
        let full = resolve_inside(&self.root, &path)?;
        if full.is_dir() {
            anyhow::bail!("路径是目录，不是文件: {path}");
        }
        // 先看一眼是不是覆盖，成功文案要能让模型分清「新建」和「覆盖」
        let existed = full.is_file();
        atomic_write(&full, &contents)?;
        let verb = if existed {
            "已覆盖写入"
        } else {
            "已写入"
        };
        Ok(format!("{verb} {path}（{} 字节）", contents.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::sandbox::TestWorkspace;

    #[tokio::test]
    async fn writes_new_file_and_overwrite() {
        let ws = TestWorkspace::new();
        let tool = WriteFile::new(ws.path());
        let created = tool
            .execute(serde_json::json!({"path": "note.txt", "contents": "你好"}))
            .await
            .unwrap();
        assert!(created.contains("已写入"), "{created}");
        assert_eq!(
            std::fs::read_to_string(ws.path().join("note.txt")).unwrap(),
            "你好"
        );
        let updated = tool
            .execute(serde_json::json!({"path": "note.txt", "contents": "two"}))
            .await
            .unwrap();
        assert!(updated.contains("已覆盖写入"), "{updated}");
        assert!(updated.contains("3 字节"), "{updated}");
        assert_eq!(
            std::fs::read_to_string(ws.path().join("note.txt")).unwrap(),
            "two"
        );
        // 空字符串是清空，不是「缺少参数」
        tool.execute(serde_json::json!({"path": "empty.txt", "contents": ""}))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(ws.path().join("empty.txt")).unwrap(),
            ""
        );
        let names: Vec<_> = std::fs::read_dir(ws.path())
            .unwrap()
            .map(|ent| ent.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names
                .iter()
                .all(|name| !name.starts_with(".mini-agent-tmp-"))
        );
    }

    #[tokio::test]
    async fn rejects_missing_parent_and_directory() {
        let ws = TestWorkspace::new();
        std::fs::create_dir(ws.path().join("dir")).unwrap();
        let tool = WriteFile::new(ws.path());
        let err = tool
            .execute(serde_json::json!({"path": "missing/a.txt", "contents": "x"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("父目录"), "{err}");
        assert!(!ws.path().join("missing").exists());
        let err = tool
            .execute(serde_json::json!({"path": "dir", "contents": "x"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("目录"), "{err}");
        let err = tool
            .execute(serde_json::json!({"path": "a.txt"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("contents"), "{err}");
    }

    #[tokio::test]
    async fn rejects_escape_without_creating_outside() {
        let ws = TestWorkspace::new();
        let outside = TestWorkspace::new();
        let name = outside
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let tool = WriteFile::new(ws.path());
        let err = tool
            .execute(serde_json::json!({
                "path": format!("../{name}/pwned.txt"),
                "contents": "x"
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        assert!(!outside.path().join("pwned.txt").exists());
    }
}
