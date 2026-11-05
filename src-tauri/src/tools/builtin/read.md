# `read.rs`

- 源码：[`read.rs`](./read.rs)
- 分层：工具层 / 内置工具

## 职责

实现 `read` 工具：按路径读取工作区内的 UTF-8 文本，支持从第几行开始、最多返回多少行，超限时截断并告诉模型下一行的 `offset`。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `ReadFile` | 持有工作区根的工具 |
| `impl Tool for ReadFile` | `name()` 返回 `"read"` |
| `read_text()` | 读文件再切片 |
| `slice_text()` | 纯函数：按 `offset` / `limit` / 字节上限切文本 |
| `logical_lines()` | 按 `\n` 分行，去掉行尾 `\r`；末尾换行不算额外空行 |
| `truncate_utf8()` | 按字节上限截断，不把汉字切成半个 |

常量：未指定 `limit` 时最多 400 行；模型把 `limit` 写得很大也封顶在 2000 行；单次正文最多 32 KiB；为了切片，文件本身超过 8 MiB 会直接拒绝。

## 逻辑要点

1. `offset` 从 1 开始。空文件配 `offset = 1` 返回空文本；更大的 `offset` 报「超出文件末尾」。
2. `limit` 和字节上限谁先到就停，并在正文后面加一行 `[已截断] ... 继续读请把 offset 设为 N`。
3. 单行比字节上限还长时，按字符边界砍断这一行，**不会从这行中间续读**。下次 `offset` 会跳到下一行。
4. 路径先过 [`resolve_inside`](../sandbox.md)。二进制（含 NUL）和目录在 [`read_utf8_file`](../sandbox.md) 里拒绝。
5. 读文件是同步的。单次有字节上限，所以没有再丢进 `spawn_blocking`。

## 单元测试

| 测试 | 锁住什么 |
| --- | --- |
| `slice_respects_offset_and_limit` | 行窗口、截断提示、CRLF、空文件 |
| `slice_truncates_on_byte_cap_and_rejects_bad_offset` | 字节截断不切开汉字；`offset` 为 0 或越界；`limit` 为 0 |
| `read_tool_reads_slice_and_rejects_escape` | 工具参数里的空白路径、`offset`/`limit`、`../`、找不到文件 |
| `read_tool_rejects_binary_directory_and_huge_offset` | NUL、目录、超出末尾 |

## 依赖关系

- 依赖：[`tools::Tool`](../mod.md)、[`sandbox`](../sandbox.md)、[`protocol`](../../protocol.md)。
- 被谁使用：[`ToolRegistry::with_workspace()`](../mod.md)。

## 当前行为说明

- 8 MiB 以上的文件整份拒绝，`offset` 也帮不上忙。更大的日志需要以后改成按行流式读。
- 工具失败走 `Err`，主循环会把它包成 `工具执行失败: ...` 回给模型，文件不会被改。
