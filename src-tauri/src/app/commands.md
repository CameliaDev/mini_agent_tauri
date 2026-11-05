# `commands.rs`

- 源码：[`commands.rs`](./commands.rs)
- 分层：Tauri 宿主层 / IPC 适配
- 拆分来源：原 `lib.rs` 中的 `greet`、`chat` 两个命令，以及原 `run_agent` 里创建内部通道、`tokio::spawn` 转发、`drop(tx)` / `await forward` 的代码

## 职责

把前端 `invoke` 调用翻译成 agent 主循环，并把内部异步通道里的事件转发到 Tauri 的前端 `Channel`。这是全项目唯一接触 `tauri::ipc::Channel` 的地方。

## 包含的定义

| 定义 | 签名 | 作用 |
| --- | --- | --- |
| `greet` | `pub(crate) fn greet(name: &str) -> String` | 模板示例命令，返回问候字符串 |
| `chat` | `pub(crate) async fn chat(state: State<'_, AppState>, on_event: Channel<StreamEvent>, req: ChatRequest) -> Result<ChatResponse, String>` | 跑完整 agent 流程，流式事件走通道推给前端 |

## 逻辑要点

1. `chat` 先建容量为 64 的内部通道 `tauri::async_runtime::channel::<StreamEvent>(64)`：`chat_stream` 往 `tx` 推，转发任务从 `rx` 取。
2. `tokio::spawn` 起一个转发任务，`while let Some(event) = rx.recv().await` 逐条 `on_event.send(event)`；前端页面关闭导致发送失败时忽略，不打断主流程。
3. 调用 `run_agent(state.config.clone(), tx.clone(), req).await`，主循环只会接触内部 `Sender`，不知道 IPC 通道的存在。
4. 主循环返回后 `drop(tx)` 释放 command 手里的发送端，`rx` 才会关闭；随后 `await forward` 等剩余事件全部推完。
5. `result.map_err(|e| e.to_string())` 把 `anyhow` 错误转成前端能显示的字符串。

事件流向：`chat_stream → tx → rx → forward → on_event(Channel) → 前端`。

## 新增命令（会话持久化 + 停止生成）

| 命令 | 参数 | 作用 |
| --- | --- | --- |
| `stop_generation` | 无 | 触发取消令牌，打断正在进行的网络读取 |
| `list_sessions` | 无 | 列出全部会话（侧栏用），按更新时间倒序 |
| `load_messages` | `session_id` | 读某个会话的全部消息 |
| `save_session` | `session` | 新建或覆盖一个会话的元信息 |
| `append_message` | `session_id` / `seq` / `message` | 追加一条消息 |
| `delete_session` | `session_id` | 删除一个会话（消息随外键级联一起删） |
| `delete_session_messages` | `session_id` / `after_id` | 删掉某条消息之后的所有消息（重新生成时用） |
| `clear_session_messages` | `session_id` | 清空消息但保留会话 |

这组命令是 [`storage`](../storage/mod.md) 存储层对外的**唯一入口**：命令层只做「取状态 → 调存储 → 转错误字符串」，不写 SQL。

参数名对应关系（Tauri 2 参数默认 camelCase，前端传驼峰、Rust 收下划线）：

| Rust | 前端 |
| --- | --- |
| `session_id` | `sessionId` |
| `after_id` | `afterId` |
| `session` | `session` |

## 停止生成是怎么打通的

1. 前端调 `stop_generation` → `state.cancel.cancel()`；
2. 正在读网络流的 [`sse.rs`](../providers/sse.md) 里 `tokio::select!` 的「被取消」分支先命中；
3. 读流循环退出，推一个 `StreamEvent::Cancelled`，把已收到的半截文字作为结果返回；
4. 前端收到 `Cancelled` → 调 `finish()` 收掉「生成中」。

`chat` 命令开头的顺序很重要：**先 `reset()` 再 `subscribe()`**，否则新一轮会读到上一次的「已取消」旧值。

## 依赖关系

- 依赖：`tauri::ipc::Channel`、[`agent::run_agent`](../agent/runner.md)、[`agent::events::StreamEvent`](../agent/events.md)、[`app::state::AppState`](./state.md)、[`protocol`](../protocol.md)。
- 被谁使用：[`lib.rs`](../lib.md) 通过 `tauri::generate_handler![greet, chat]` 注册，前端用 `invoke` 调用。

## 当前行为说明

- 两个命令是 `pub(crate)`（不是 `pub`）：因为要从 `lib.rs` 跨模块 `use` 后交给宏注册。
- 运行出错时通道上不会收到 `Done` 或错误事件，前端只能通过 `invoke` 的 Promise reject 拿到字符串错误。
- `greet` 是模板命令，代码注释里标注"保留不动"。

## 怎么读这个文件

按这 6 步读，每一步都对应函数里的一段注释：

1. 建内部通道（容量 64）；
2. 起转发任务，把内部通道的事件搬到前端的 `Channel`；
3. 把活交给 `run_agent`；
4. `drop(tx)`：所有发送端都释放了，`rx` 才会结束；
5. `await forward`：等最后几个事件推完再返回；
6. `map_err`：把 `anyhow` 错误变成字符串交给前端。

动手复刻提示：第 4、5 步最容易漏。少了 `drop(tx)`，转发任务永远等不到结束，`await` 会一直挂住；少了 `await forward`，界面会丢掉最后几个字。
