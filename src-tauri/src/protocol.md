# `protocol.rs`

- 源码：[`protocol.rs`](./protocol.rs)
- 分层：协议 / DTO 层
- 拆分来源：原 `models/model.rs` 里的数据结构部分（配置类型已移到 `providers/mod.rs`）

## 职责

定义前后端、模型厂商、工具之间共用的数据结构。全是数据、没有逻辑，也不依赖 Tauri 或任何厂商 SDK，是各层之间最底层的公共语言。

## 包含的定义

| 定义 | 作用 | 关键字段 / serde 属性 |
| --- | --- | --- |
| `ChatMessage` | 一条对话消息 | `role`、`content`、`tool_calls`、`tool_call_id`；两个可选字段用 `skip_serializing_if = "Option::is_none"`，为 `None` 时不出现在 JSON 里 |
| `ChatRequest` | 前端发来的聊天请求 | `messages`、`model`、`temperature`、`tools`；`tools` 带 `#[serde(default)]`，前端不传就是空列表 |
| `ChatResponse` | 一轮模型调用的汇总 | `content`、`usage_tokens`、`tool_calls`、`finish_reason` |
| `ToolDefinition` | 发给模型的工具说明书 | `#[serde(rename = "type")] kind`（Rust 关键字规避）、`function` |
| `ToolFunction` | 说明书里的函数部分 | `name`、`description`、`parameters`（JSON Schema，用 `serde_json::Value` 装任意结构） |
| `ToolCall` | 模型发起的一次工具调用 | `id`、`#[serde(rename = "type")] kind`、`function` |
| `ToolCallFunction` | 工具调用里的函数名与参数 | `name`、`arguments`（注意是 **JSON 字符串**，不是对象，要自己 parse） |

## 逻辑要点

1. `role` 取 `system / user / assistant / tool`；只有 `assistant` 消息带 `tool_calls`，只有 `tool` 消息带 `tool_call_id`，两者靠 `id` 对应。
2. `kind` 在 Rust 里规避关键字 `type`，序列化时通过 `rename` 还原成协议要求的 `"type"`，值固定为 `"function"`。
3. `arguments` 在流式返回中是一段段拼接出来的字符串，执行工具前才解析成 `serde_json::Value`。

## 依赖关系

- 依赖：`serde`、`serde_json`。
- 被谁使用：[`app/commands.rs`](./app/commands.rs)、[`agent/events.rs`](./agent/events.rs)、[`agent/session.rs`](./agent/session.rs)、[`agent/runner.rs`](./agent/runner.rs)、[`providers/`](./providers/mod.md)、[`tools/`](./tools/mod.md)。

## 当前行为说明

`usage_tokens` 目前恒为 `None`：请求体没有带 `stream_options.include_usage`，也没有统计逻辑。


## 持久化相关结构（新增）

这一层多了一组专门给 SQLite 用的结构，和上面的对话结构分开：

| 定义 | 对应数据库 | 说明 |
| --- | --- | --- |
| `SessionMeta` | `sessions` 表的一行 | 会话元信息：id / title / created_at / updated_at |
| `StoredMessage` | `messages` 表的一行 | 落库的消息：id / role / content / tool_name / tool_call / created_at |

两者的字段名都是数据库列名（snake_case），前后端 JSON 里也是这几个名字，这样对照着看最直观。

`StoredMessage.tool_call` 用的是 `Option<serde_json::Value>`：工具调用本身是结构化的，落库时序列化成 JSON 文本，读出来再解析回来。
