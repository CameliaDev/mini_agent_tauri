# `session.rs`

- 源码：[`session.rs`](./session.rs)
- 分层：agent 运行时层 / 会话历史
- 拆分来源：原 `lib.rs` 的 `run_agent` 里两处 `req.messages.push(...)`

## 职责

把「一轮模型回复」和「一条工具执行结果」转换成协议要求的消息，追加进对话历史。只是两个无状态的辅助函数，不持有状态、不做持久化。

## 包含的定义

| 定义 | 签名 | 作用 |
| --- | --- | --- |
| `append_assistant` | `fn(&mut Vec<ChatMessage>, &ChatResponse)` | 追加一条 `role = "assistant"` 的消息 |
| `append_tool_result` | `fn(&mut Vec<ChatMessage>, &ToolCall, String)` | 追加一条 `role = "tool"` 的消息 |

## 逻辑要点

1. `append_assistant` 复制本轮的 `content` 与 `tool_calls`，`tool_calls` 包成 `Some(...)`；`tool_call_id` 置 `None`。第二轮请求必须把这条消息带上，模型才知道自己点过哪些工具。
2. `append_tool_result` 把工具输出放进 `content`，`tool_calls` 置 `None`，并用 `call.id.clone()` 填 `tool_call_id`——这个 id 是和上面 `ToolCall.id` 对上的唯一依据。
3. 两者都只 `push`，不修改已有消息，也不裁剪上下文长度。

## 依赖关系

- 依赖：[`crate::protocol`](../protocol.md) 的 `ChatMessage`、`ChatResponse`、`ToolCall`。
- 被谁使用：[`agent/runner.rs`](./runner.md) 的主循环。

## 当前行为说明

这里还没有"会话"对象：历史就是 `ChatRequest.messages` 这个 `Vec`，没有保存、恢复、窗口压缩；`Cargo.toml` 里的 `rusqlite` 目前未使用。
