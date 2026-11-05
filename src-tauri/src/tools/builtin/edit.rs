// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入路径类型：工具要记住自己的工作区根
use std::path::PathBuf;
// 引入工具说明书相关的数据结构
use crate::protocol::{ToolDefinition, ToolFunction};
// 引入沙箱：解析路径、读全文、原子写回
use crate::tools::sandbox::{arg_path, arg_string, atomic_write, read_utf8_file, resolve_inside};
// 引入工具统一接口
use crate::tools::Tool;

// edit 要整文件载入才能判断「是不是唯一匹配」。超过这个大小就拒绝，免得一次读爆内存
const MAX_EDIT_BYTES: u64 = 2 * 1024 * 1024;

// 定义内置工具：把文件里唯一的一段文本替换掉
pub struct EditFile {
    // 工作区根。注册表构造工具时塞进来
    root: PathBuf,
}

impl EditFile {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

// 匹配失败的原因。用枚举是为了测试能精确断言，而不是去拆错误字符串
#[derive(Debug, PartialEq, Eq)]
enum EditReject {
    EmptyOld,
    Identical,
    NotFound,
    Multiple(usize),
}

impl std::fmt::Display for EditReject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // 空串会在每个位置都「匹配」，必须单独拒绝
            Self::EmptyOld => write!(f, "old_string 不能为空"),
            Self::Identical => write!(f, "old_string 与 new_string 相同，没有需要修改的内容"),
            Self::NotFound => write!(f, "未找到要替换的文本"),
            Self::Multiple(n) => write!(
                f,
                "找到 {n} 处匹配，edit 要求唯一匹配。请提供更长的 old_string 以唯一定位"
            ),
        }
    }
}

impl std::error::Error for EditReject {}

// 只在恰好一处（不重叠）匹配时替换。零处或多处都不改内容。
fn apply_unique_edit(content: &str, old: &str, new: &str) -> Result<String, EditReject> {
    if old.is_empty() {
        return Err(EditReject::EmptyOld);
    }
    if old == new {
        return Err(EditReject::Identical);
    }
    let count = count_non_overlapping(content, old);
    match count {
        0 => Err(EditReject::NotFound),
        1 => Ok(content.replacen(old, new, 1)),
        n => Err(EditReject::Multiple(n)),
    }
}

// 不重叠计数，和 replacen 的行为一致：`"aaa"` 里的 `"aa"` 只算 1 处，不是 2 处。
fn count_non_overlapping(haystack: &str, needle: &str) -> usize {
    let mut count = 0usize;
    let mut start = 0usize;
    while let Some(pos) = haystack[start..].find(needle) {
        count += 1;
        // find 返回的是字节下标，needle.len() 也是字节，UTF-8 边界仍然对齐
        start += pos + needle.len();
    }
    count
}

// 为它实现 Tool 接口
#[async_trait]
impl Tool for EditFile {
    fn name(&self) -> &'static str {
        "edit"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function".to_string(),
            function: ToolFunction {
                name: self.name().to_string(),
                description: "在工作区的文本文件中，把唯一出现的 old_string 替换成 new_string。old_string 必须和文件内容完全一致（包括空格和换行）。找不到或找到多处时会报错，并且不会修改文件。path 不能逃出工作区。".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "相对工作区根目录的文件路径"
                        },
                        "old_string": {
                            "type": "string",
                            "description": "要被替换的原文，必须在文件中恰好出现一次"
                        },
                        "new_string": {
                            "type": "string",
                            "description": "替换成的新文本，空字符串表示删掉这段原文"
                        }
                    },
                    "required": ["path", "old_string", "new_string"]
                }),
            },
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String> {
        let path = arg_path(&args, "path")?;
        let old = arg_string(&args, "old_string")?;
        let new = arg_string(&args, "new_string")?;
        let full = resolve_inside(&self.root, &path)?;
        // 先读全文。匹配失败就在写回之前返回，文件保持原样
        let content = read_utf8_file(&full, MAX_EDIT_BYTES)?;
        let updated = apply_unique_edit(&content, &old, &new)?;
        atomic_write(&full, &updated)?;
        Ok(format!("已替换 {path} 中的 1 处匹配"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::sandbox::TestWorkspace;

    #[test]
    fn non_overlapping_count_and_reject_reasons() {
        // "aaa" 里不重叠的 "aa" 只有一处，替换后剩下最后一个 a
        assert_eq!(apply_unique_edit("aaa", "aa", "b").unwrap(), "ba");
        assert_eq!(
            apply_unique_edit("ab ab", "ab", "x"),
            Err(EditReject::Multiple(2))
        );
        assert_eq!(apply_unique_edit("ab", "", "x"), Err(EditReject::EmptyOld));
        assert_eq!(
            apply_unique_edit("ab", "ab", "ab"),
            Err(EditReject::Identical)
        );
        assert_eq!(
            apply_unique_edit("ab", "zzz", "x"),
            Err(EditReject::NotFound)
        );
    }

    #[tokio::test]
    async fn replaces_one_match_including_newlines() {
        let ws = TestWorkspace::new();
        ws.write("a.txt", "你好，世界\nsecond\n");
        let tool = EditFile::new(ws.path());
        let out = tool
            .execute(serde_json::json!({
                "path": "a.txt",
                "old_string": "世界\nsecond",
                "new_string": "同学\nSECOND"
            }))
            .await
            .unwrap();
        assert!(out.contains("已替换"), "{out}");
        assert_eq!(
            std::fs::read_to_string(ws.path().join("a.txt")).unwrap(),
            "你好，同学\nSECOND\n"
        );
    }

    #[tokio::test]
    async fn zero_or_many_matches_do_not_change_file() {
        let ws = TestWorkspace::new();
        ws.write("a.txt", "one one");
        let tool = EditFile::new(ws.path());
        let before = std::fs::read_to_string(ws.path().join("a.txt")).unwrap();
        let err = tool
            .execute(serde_json::json!({
                "path": "a.txt",
                "old_string": "one",
                "new_string": "two"
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("2 处"), "{err}");
        assert_eq!(
            std::fs::read_to_string(ws.path().join("a.txt")).unwrap(),
            before
        );
        let err = tool
            .execute(serde_json::json!({
                "path": "a.txt",
                "old_string": "missing",
                "new_string": "x"
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("未找到"), "{err}");
        assert_eq!(
            std::fs::read_to_string(ws.path().join("a.txt")).unwrap(),
            before
        );
        let err = tool
            .execute(serde_json::json!({
                "path": "a.txt",
                "old_string": "",
                "new_string": "x"
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("不能为空"), "{err}");
        assert_eq!(
            std::fs::read_to_string(ws.path().join("a.txt")).unwrap(),
            before
        );
    }

    #[tokio::test]
    async fn rejects_escape() {
        let ws = TestWorkspace::new();
        let outside = TestWorkspace::new();
        outside.write("secret.txt", "keep");
        let tool = EditFile::new(ws.path());
        let name = outside
            .path()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let err = tool
            .execute(serde_json::json!({
                "path": format!("../{name}/secret.txt"),
                "old_string": "keep",
                "new_string": "nope"
            }))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        assert_eq!(
            std::fs::read_to_string(outside.path().join("secret.txt")).unwrap(),
            "keep"
        );
    }
}
