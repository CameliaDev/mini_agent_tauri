# `events.rs`

- 源码：[`events.rs`](./events.rs)
- 分层：agent 运行时层 / 事件协议
- 拆分来源：原 `llm/mod.rs` 中的 `StreamEvent`

## 职责

定义内部通道和 Tauri IPC 通道上流动的事件类型。模型层只负责产生事件，宿主层只负责转发，两边通过这个枚举解耦。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `StreamEvent` | `#[derive(Debug, Clone, Serialize)]` 的枚举，是所有流式事件的统一载体 |
| `TextDelta(String)` | 一小段正文文字 |
| `ToolCalls(Vec<ToolCall>)` | 一轮流解析完后汇总出的工具调用订单 |
| `ToolResultEvent` | 一个工具跑完：`call_id`、`name`、`output`、`is_error` |
| `ToolResult(ToolResultEvent)` | 把上面的结果推给前端，用来填工具卡片 |
| `Cancelled` | 用户点了停止，本轮提前结束 |
| `Done` | 整个 agent 流程结束 |

## 逻辑要点

1. 枚举用 serde 的默认外部标签表示，序列化后的前端形态是：`TextDelta` → `{"TextDelta":"..."}`，`ToolCalls` → `{"ToolCalls":[...]}`，`ToolResult` → `{"ToolResult":{...}}`，`Cancelled` 和 `Done` 是字符串 `"Cancelled"` / `"Done"`。前端订阅 `Channel<StreamEvent>` 时要按这个形状判断。
2. `Clone` 是为了在内部 `mpsc` 通道与前端通道之间搬运、以及 `ToolCalls` 的 `pending_calls.clone()`；`Serialize` 是 Tauri `Channel` 的要求。
3. `ToolCalls` 里装的是 [`ToolCall`](../protocol.md)，即协议层的 DTO。

## 依赖关系

- 依赖：`serde::Serialize`、`crate::protocol::ToolCall`。
- 被谁使用：[`app/commands.rs`](../app/commands.md)（`Channel<StreamEvent>` 与内部通道）、[`providers/sse.rs`](../providers/sse.md)（推 `TextDelta` / `ToolCalls`）、[`agent/runner.rs`](./runner.md)（推 `ToolResult` / `Cancelled` / `Done`）、[`providers/mod.rs`](../providers/mod.md)（trait 的参数类型）。

## 当前行为说明

通道上有文字、工具订单、工具结果、取消和结束。运行出错时前端仍然从 `invoke` 的 Promise reject 拿到字符串错误，通道上没有单独的 `Error` 事件。`is_error` 只表示工具自己失败（输出以「工具执行失败」开头），不是整轮失败。
