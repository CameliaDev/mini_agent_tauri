// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入路径类型：工具要记住自己的工作区根
use std::path::PathBuf;
// 引入工具说明书相关的数据结构
use crate::protocol::{ToolDefinition, ToolFunction};
// 引入沙箱：路径检查和读文本都在那里
use crate::tools::sandbox::{arg_path, arg_usize_opt, read_utf8_file, resolve_inside};
// 引入工具统一接口
use crate::tools::Tool;

// 没写 limit 时最多返回这么多行，避免一次把大文件塞进模型上下文
const DEFAULT_MAX_LINES: usize = 400;
// 模型把 limit 写成很大的数时也要封顶
const MAX_LINES_CAP: usize = 2_000;
// 单次返回正文的字节上限。截断发生在行边界，除了「单行本身就超限」
const MAX_READ_BYTES: usize = 32 * 1024;
// 为了按行切片，先把文件读进内存。超过这个大小直接拒绝，而不是悄悄读几百 MB
const MAX_SCAN_BYTES: u64 = 8 * 1024 * 1024;

// 定义内置工具：读取工作区内的文本文件
pub struct ReadFile {
    // 工作区根。注册表构造工具时塞进来，执行时不再读环境变量
    root: PathBuf,
}

impl ReadFile {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

// 为它实现 Tool 接口
#[async_trait]
impl Tool for ReadFile {
    fn name(&self) -> &'static str {
        "read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            kind: "function".to_string(),
            function: ToolFunction {
                name: self.name().to_string(),
                description: "读取工作区内的 UTF-8 文本文件。path 相对工作区根目录，不能用 .. 逃出根目录。offset 是起始行，从 1 开始，默认 1。limit 是最多返回的行数，默认 400。内容过长会被截断，返回文本末尾会说明下一次该用的 offset。".to_string(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "相对工作区根目录的文件路径"
                        },
                        "offset": {
                            "type": "integer",
                            "description": "从第几行开始读，从 1 起计，默认 1"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "最多返回多少行，默认 400"
                        }
                    },
                    "required": ["path"]
                }),
            },
        }
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String> {
        // 路径去掉空白；offset / limit 缺省时用上面的常量
        let path = arg_path(&args, "path")?;
        let offset = arg_usize_opt(&args, "offset")?.unwrap_or(1);
        let limit = arg_usize_opt(&args, "limit")?
            .unwrap_or(DEFAULT_MAX_LINES)
            .min(MAX_LINES_CAP);
        let full = resolve_inside(&self.root, &path)?;
        // 读文件是同步的。单次有字节上限，先不占一个阻塞线程池
        let text = read_text(&full, offset, limit, MAX_READ_BYTES)?;
        Ok(text.render())
    }
}

// 一次读取的结果。truncated 为真时，render 会附上「下一行从哪继续」
#[derive(Debug)]
struct ReadText {
    body: String,
    start_line: usize,
    lines_returned: usize,
    truncated: bool,
    next_offset: usize,
}

impl ReadText {
    fn render(&self) -> String {
        if !self.truncated {
            return self.body.clone();
        }
        let note = format!(
            "[已截断] 本次从第 {} 行起返回了 {} 行。继续读请把 offset 设为 {}。",
            self.start_line, self.lines_returned, self.next_offset
        );
        if self.body.is_empty() {
            note
        } else {
            format!("{}\n\n{note}", self.body)
        }
    }
}

// 读文件再按行切片。offset 从 1 开始；limit 和 max_bytes 谁先到就停。
fn read_text(
    path: &std::path::Path,
    offset: usize,
    limit: usize,
    max_bytes: usize,
) -> anyhow::Result<ReadText> {
    if offset == 0 {
        anyhow::bail!("offset 从 1 开始");
    }
    if limit == 0 {
        anyhow::bail!("limit 必须大于 0");
    }
    let text = read_utf8_file(path, MAX_SCAN_BYTES)?;
    slice_text(&text, offset, limit, max_bytes)
}

// 纯函数：给定全文，切出要返回的那一段。测试不用碰磁盘。
fn slice_text(
    text: &str,
    offset: usize,
    limit: usize,
    max_bytes: usize,
) -> anyhow::Result<ReadText> {
    if offset == 0 {
        anyhow::bail!("offset 从 1 开始");
    }
    if limit == 0 {
        anyhow::bail!("limit 必须大于 0");
    }
    if max_bytes == 0 {
        anyhow::bail!("读取字节上限必须大于 0");
    }
    let lines = logical_lines(text);
    // 空文件：offset 1 是「从头读」，结果就是空；更大的 offset 才算越界
    if lines.is_empty() {
        if offset == 1 {
            return Ok(ReadText {
                body: String::new(),
                start_line: 1,
                lines_returned: 0,
                truncated: false,
                next_offset: 1,
            });
        }
        anyhow::bail!("offset {offset} 超出文件末尾（文件共 0 行）");
    }
    let start = offset - 1;
    if start >= lines.len() {
        anyhow::bail!("offset {offset} 超出文件末尾（文件共 {} 行）", lines.len());
    }
    let mut body = String::new();
    let mut returned = 0usize;
    let mut truncated = false;
    for line in lines.iter().skip(start) {
        if returned >= limit {
            truncated = true;
            break;
        }
        // 第一行自己就超限：按字符边界砍断，并且不再往后读
        if returned == 0 && line.len() > max_bytes {
            body.push_str(truncate_utf8(line, max_bytes));
            returned = 1;
            truncated = true;
            break;
        }
        let extra = if returned == 0 {
            line.len()
        } else {
            line.len() + 1
        };
        if returned > 0 && body.len() + extra > max_bytes {
            truncated = true;
            break;
        }
        if returned > 0 {
            body.push('\n');
        }
        body.push_str(line);
        returned += 1;
    }
    Ok(ReadText {
        body,
        start_line: offset,
        lines_returned: returned,
        truncated,
        // 下一次从「还没返回的那一行」开始。单行被砍断时，剩余半行不会续上
        next_offset: offset + returned,
    })
}

// 按 `\n` 分行，并去掉行尾的 `\r`。末尾的换行不算额外的空行：`"a\n"` 只有一行。
fn logical_lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut rest = text;
    while let Some(idx) = rest.find('\n') {
        let (raw, after) = rest.split_at(idx);
        lines.push(raw.strip_suffix('\r').unwrap_or(raw));
        rest = &after[1..];
    }
    if !text.ends_with('\n') {
        lines.push(rest.strip_suffix('\r').unwrap_or(rest));
    }
    lines
}

// 字节上限可能落在汉字中间，往回退到字符边界再切。
fn truncate_utf8(text: &str, max_bytes: usize) -> &str {
    if text.len() <= max_bytes {
        return text;
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::sandbox::TestWorkspace;

    #[test]
    fn slice_respects_offset_and_limit() {
        let text = "L1\nL2\nL3\nL4\nL5\n";
        let mid = slice_text(text, 2, 2, 1024).unwrap();
        assert_eq!(mid.body, "L2\nL3");
        assert!(mid.truncated);
        assert_eq!(mid.next_offset, 4);
        assert!(mid.render().contains("offset 设为 4"));
        let tail = slice_text(text, 4, 10, 1024).unwrap();
        assert_eq!(tail.body, "L4\nL5");
        assert!(!tail.truncated);
        assert_eq!(tail.render(), "L4\nL5");
        // 只有换行的文件算一行空行；末尾换行不另算一行
        assert_eq!(logical_lines("a\r\nb\n"), vec!["a", "b"]);
        assert_eq!(logical_lines("\n"), vec![""]);
        let empty = slice_text("", 1, 10, 1024).unwrap();
        assert_eq!(empty.body, "");
        assert!(!empty.truncated);
    }

    #[test]
    fn slice_truncates_on_byte_cap_and_rejects_bad_offset() {
        let text = "hello\nworld\n";
        let cut = slice_text(text, 1, 10, 4).unwrap();
        assert_eq!(cut.body, "hell");
        assert!(cut.truncated);
        assert_eq!(cut.next_offset, 2);
        // 汉字不能被切成半个
        let zh = slice_text("你好", 1, 10, 4).unwrap();
        assert_eq!(zh.body, "你");
        assert!(zh.truncated);
        let err = slice_text(text, 0, 1, 10).unwrap_err();
        assert!(err.to_string().contains("从 1 开始"));
        let err = slice_text(text, 5, 1, 10).unwrap_err();
        assert!(err.to_string().contains("超出文件末尾"));
        let err = slice_text("", 2, 1, 10).unwrap_err();
        assert!(err.to_string().contains("共 0 行"));
        let err = slice_text(text, 1, 0, 10).unwrap_err();
        assert!(err.to_string().contains("limit"));
    }

    #[tokio::test]
    async fn read_tool_reads_slice_and_rejects_escape() {
        let ws = TestWorkspace::new();
        ws.write("t.txt", "L1\nL2\nL3\nL4\nL5\n");
        let tool = ReadFile::new(ws.path());
        let out = tool
            .execute(serde_json::json!({"path": "  t.txt  ", "offset": 2, "limit": 2}))
            .await
            .unwrap();
        assert!(out.starts_with("L2\nL3"), "{out}");
        assert!(out.contains("offset 设为 4"), "{out}");
        let tail = tool
            .execute(serde_json::json!({"path": "t.txt", "offset": 4, "limit": 10}))
            .await
            .unwrap();
        assert_eq!(tail, "L4\nL5");
        let err = tool
            .execute(serde_json::json!({"path": "../secret"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("逃出"), "{err}");
        let err = tool
            .execute(serde_json::json!({"path": "missing.txt"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("找不到"), "{err}");
    }

    #[tokio::test]
    async fn read_tool_rejects_binary_directory_and_huge_offset() {
        let ws = TestWorkspace::new();
        std::fs::write(ws.path().join("bin.dat"), b"a\0b").unwrap();
        std::fs::create_dir(ws.path().join("dir")).unwrap();
        let tool = ReadFile::new(ws.path());
        let err = tool
            .execute(serde_json::json!({"path": "bin.dat"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("二进制"), "{err}");
        let err = tool
            .execute(serde_json::json!({"path": "dir"}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("目录"), "{err}");
        ws.write("one.txt", "only\n");
        let err = tool
            .execute(serde_json::json!({"path": "one.txt", "offset": 3}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("超出文件末尾"), "{err}");
    }
}
