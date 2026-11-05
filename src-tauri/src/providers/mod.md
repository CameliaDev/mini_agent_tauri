# `mod.rs`（providers 模块）

- 源码：[`mod.rs`](./mod.rs)
- 分层：模型适配层入口
- 拆分来源：原 `llm/mod.rs` 的 `ChatModel`、原 `llm/chat.rs` 的 `create_provider`、原 `models/model.rs` 的 `ModelProvider` 与 `ProviderConfig`

## 职责

定义「模型接口 + 厂商地址 + 服务商配置 + 适配器工厂」，让 agent 主循环只面向 `ChatModel` trait 编程，不关心具体是哪家厂商、怎么发 HTTP。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub mod openai_compat` | OpenAI 兼容协议适配器，见 [`openai_compat.md`](./openai_compat.md) |
| `pub mod sse` | SSE 字节流解析器，见 [`sse.md`](./sse.md) |
| `ModelProvider` | 厂商枚举：`DeepSeek` / `OpenAi` / `Qwen`，带 `#[serde(rename_all = "snake_case")]` |
| `ModelProvider::base_url()` | DeepSeek → `https://api.deepseek.com`；OpenAI → `https://api.openai.com/v1`；Qwen → `https://dashscope.aliyuncs.com/compatible-mode/v1` |
| `ProviderConfig` | `{ provider: ModelProvider, api_key: String }`，`#[derive(Debug, Clone)]` |
| `ChatModel` | `#[async_trait]` trait，约束 `Send + Sync`，只有一个 `chat_stream` 方法 |
| `create_provider` | 工厂函数，返回 `Box<dyn ChatModel>` |

## 逻辑要点

`chat_stream` 的第三个参数是取消信号（[`CancelSignal`](../agent/cancel.md)）：它是「协作式取消」的载体，适配器要把它透传给真正阻塞的 IO 操作（SSE 解析器），用户点「停止」时才能真的打断网络读取，而不是等模型说完。


1. `chat_stream(&self, req: &ChatRequest, tx: Sender<StreamEvent>) -> anyhow::Result<ChatResponse>`：`tx` 取得所有权，方法内部可以反复 `send`；返回的是本轮汇总结果，与流式事件并行存在。
2. 用 `async_trait` 是因为 trait 里有 `async fn`；`Send + Sync` 是 Tauri 异步任务跨线程调度的要求。
3. `create_provider(&config)` 目前三家都返回 `openai_compat::OpenAiCompatProvider::new(config)`，因为都兼容 OpenAI 协议。

## 依赖关系

- 依赖：[`protocol`](../protocol.md)、[`agent/events`](../agent/events.md)、`tauri::async_runtime::Sender`、`serde`、`async_trait`。
- 被谁使用：[`agent/runner.rs`](../agent/runner.md)（创建适配器并调用）、[`app/state.rs`](../app/state.md)（装配配置）。

## 当前行为说明

`ModelProvider` 虽然列了三家，但 [`AppState::from_env()`](../app/state.md) 固定使用 `DeepSeek`，`create_provider` 也不按厂商分支；另外两个枚举值暂时没有调用路径。
