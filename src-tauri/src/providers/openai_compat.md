# `openai_compat.rs`

- 源码：[`openai_compat.rs`](./openai_compat.rs)
- 分层：模型适配层 / HTTP 请求
- 拆分来源：原 `llm/chat.rs` 的 `OpenAiCompatProvider` 与 `chat_stream` 前半段（SSE 解析已移到 `sse.rs`）

## 职责

把统一的 `ChatRequest` 变成 OpenAI 兼容协议的 `/chat/completions` 请求，带上鉴权发出去，再把响应交给 SSE 解析器。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub struct OpenAiCompatProvider` | 适配器，含 `client: reqwest::Client`、`base_url: &'static str`、`api_key: String` |
| `OpenAiCompatProvider::new(&ProviderConfig)` | 新建 `reqwest::Client`、按厂商取 `base_url()`、克隆 API Key |
| `impl ChatModel for OpenAiCompatProvider` | 实现 `chat_stream` |

## 逻辑要点

`chat_stream` 除了发请求，还负责把取消信号**透传**给 SSE 解析器：`sse::parse_response(resp, &tx, cancel)`。透传这一下是「停止生成」能真正打断网络读取的关键——如果适配器自己吞掉这个参数，`tokio::select!` 就永远等不到取消分支。


1. 请求体用 `serde_json::json!` 构造四个键：`model`、`messages`、`stream: true`、`temperature`（`unwrap_or(0.7)`）。
2. 只有 `req.tools` 非空才插入 `tools` 字段，避免某些模型对空数组报错。
3. `POST {base_url}/chat/completions`，用 `.bearer_auth(&self.api_key)` 加鉴权头，`.json(&body)` 设置请求体，`.send().await?` 发送。
4. `error_for_status()?` 把 4xx/5xx 转成错误，避免把错误页当成 SSE 流解析。
5. 最后 `sse::parse_response(resp, &tx).await`：解析逻辑不在这个文件里。

## 依赖关系

- 依赖：[`providers/mod.rs`](./mod.rs) 的 `ChatModel` / `ProviderConfig`、[`sse.rs`](./sse.md)、[`agent/events`](../agent/events.md)、[`protocol`](../protocol.md)、`reqwest`、`async_trait`。
- 被谁使用：[`create_provider`](./mod.md) 构造它；[`agent/runner.rs`](../agent/runner.md) 通过 trait 调用。

## 当前行为说明

`reqwest::Client::new()` 没有配置连接超时 / 整体超时；`client` 内部带连接池，每次请求复用。
