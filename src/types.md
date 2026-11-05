# `types.ts`

- 源码：[`types.ts`](./types.ts)
- 分层：前端协议层

## 职责

定义前端用到的、与后端 `src-tauri/src/protocol.rs` 一一对应的数据结构。纯类型，没有运行时代码。

## 包含的定义

| 定义 | 对应后端 | 说明 |
| --- | --- | --- |
| `ToolCall` | `protocol.rs` 的 `ToolCall` | Rust 里的 `kind` 序列化后叫 `type`；`function.arguments` 是 **JSON 字符串**，不是对象 |
| `ChatMessage` | `protocol.rs` 的 `ChatMessage` | `tool_calls` / `tool_call_id` 是可选字段 |
| `ChatRequest` | `protocol.rs` 的 `ChatRequest` | `tools` 不用传，后端会用自己的工具清单覆盖 |
| `ChatResponse` | `protocol.rs` 的 `ChatResponse` | `invoke("chat")` 的返回类型 |
| `ToolResultEvent` | `agent/events.rs` 的 `ToolResultEvent` | 一个工具跑完：`call_id` / `name` / `output` / `is_error` |
| `StreamEvent` | `agent/events.rs` 的 `StreamEvent` | 后端流式事件的联合类型 |
| `SessionMeta` | `protocol.rs` 的 `SessionMeta` | 会话元信息，对应数据库 `sessions` 表一行 |
| `StoredMessage` | `protocol.rs` 的 `StoredMessage` | 落库的消息，对应数据库 `messages` 表一行 |

## 为什么 `StreamEvent` 写成一个联合类型

Rust 的枚举在 serde 默认的「外部标签」表示下，JSON 形态是：

| Rust | JSON |
| --- | --- |
| `TextDelta(String)` | `{"TextDelta":"..."}` |
| `ToolCalls(Vec<ToolCall>)` | `{"ToolCalls":[...]}` |
| `ToolResult(ToolResultEvent)` | `{"ToolResult":{...}}` |
| `Cancelled`（单元变体） | `"Cancelled"` |
| `Done`（单元变体） | `"Done"` |

所以前端对应地写成 `| { TextDelta: string } | { ToolCalls: ToolCall[] } | { ToolResult: ToolResultEvent } | "Cancelled" | "Done"`：对象变体用 `in` 判断，字符串变体直接比较。

## 依赖关系

- 依赖：无（只用了 TypeScript 内置类型）。
- 被谁使用：`store.ts`、`ipc/chat.ts`、`components/MessageList.tsx`。

## 当前行为说明

- 后端 `ChatResponse.usage_tokens` 恒为 `null`、`finish_reason` 目前没有参与前端逻辑。
- `SessionMeta` / `StoredMessage` 的字段名是 snake_case（和数据库列名一致），与本文件其它结构体不同——这是刻意的，对照数据库最直观。
