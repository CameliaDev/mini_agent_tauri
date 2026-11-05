# `runner.rs`

- 源码：[`runner.rs`](./runner.rs)
- 分层：agent 运行时层 / 主循环
- 拆分来源：原 `lib.rs` 里的 `async fn run_agent`

## 职责

整个应用的核心：驱动「模型输出 → 执行工具 → 把结果回填 → 再问模型」这个循环，直到模型不再调用工具或达到轮数上限。

## 包含的定义

| 定义 | 签名 |
| --- | --- |
| `run_agent` | `pub async fn run_agent(config: ProviderConfig, tx: Sender<StreamEvent>, mut req: ChatRequest, cancel: CancelSignal, workspace_root: PathBuf) -> anyhow::Result<ChatResponse>` |

## 逻辑要点

1. 开头用传入的 `workspace_root` 和 `cancel.clone()` 创建 `ToolRegistry::with_workspace_and_cancel(...)`，再 `create_provider(&config)`，并把 `registry.definitions()` 覆盖写入 `req.tools`。文件工具和 bash 整轮共用这一个根，循环中途不重读环境变量。bash 拿到的是同一份取消信号。
2. `for _ in 0..10` 是硬上限，防止模型反复点工具导致死循环。
3. 每轮调用 `provider.chat_stream(&req, tx.clone(), cancel.clone())`，流式事件通过传入的 `tx` 进入通道；返回值 `ChatResponse` 里带着本轮完整文字、工具调用列表和 `finish_reason`。
4. `result.tool_calls.is_empty()` 表示模型给出最终答复：推一个 `StreamEvent::Done`，直接 `return Ok(result)`。
5. 有工具调用时先用 [`append_assistant`](./session.md) 把助手消息（含 `tool_calls`）追加进历史，然后逐个 `registry.execute(call)`；失败时不中断，把 `工具执行失败: {e}` 当成工具结果文本回灌给模型，让它自己决定下一步。
6. 每个工具结果用 [`append_tool_result`](./session.md) 追加成 `role = "tool"` 消息，并带上 `tool_call_id`。
7. 走满 10 轮仍没结束：推 `Done`，返回 `Err("工具调用轮数超过上限 10")`，不静默返回空结果。

## 依赖关系

- 依赖：[`cancel.rs`](./cancel.md) 的 `CancelSignal`（取消检查点要用）、[`providers`](../providers/mod.md)（`create_provider`、`ProviderConfig`）、[`tools`](../tools/mod.md)（`ToolRegistry`）、[`agent/events.rs`](./events.rs)、[`agent/session.rs`](./session.rs)、[`protocol`](../protocol.md)、`tauri::async_runtime::Sender`。
- 被谁使用：[`app/commands.rs`](../app/commands.md) 的 `chat` 命令。

## 当前行为说明

- 拆分后主循环**不再依赖** `tauri::ipc::Channel`：内部通道的创建、转发任务和收尾都在 `app/commands.rs`，这里只管往 `tx` 里发事件。
- `chat_stream` 出错时用 `?` 直接返回，此时不会发 `Done`；错误最终变成 `invoke` 的字符串错误。
- `finish_reason` 采集了但没有参与循环决策，是否继续只看 `tool_calls` 是否为空。

## 怎么读这个文件

源码里用 `1) 2) 3)` 标出了循环的几个阶段，照这个顺序读即可：

1. **准备**：注册表（菜单）+ 适配器（跑腿的），把工具清单注入请求；
2. **循环**：一轮 = 一次 `chat_stream`；
3. **分两种结局**：没有工具调用就收工，有工具调用就继续；
4. **把模型的回复放回历史**：`append_assistant`；
5. **执行工具并把结果放回历史**：`append_tool_result`，其中 `tool_call_id` 是模型把结果和那次调用对起来的唯一凭据。

动手复刻提示：先写成「只发一次请求、拿到最终答复」的版本（把工具那几行注释掉），跑通后再把工具循环打开。这样每一步都能单独验证，出错时好定位。


## 取消检查点

主循环里有两处会检查取消信号。bash 在执行过程中还有一处，因为它是唯一会自己等信号的工具：

| 位置 | 为什么在那里 |
| --- | --- |
| 每轮开始前 | 工具执行完可能还要跑好几秒，那段时间里用户可能改主意了 |
| 每个工具执行前 | 模型点了工具但用户在这期间点了停止，就别白跑工具了 |
| bash 执行过程中 | 注册表把同一份信号交给 [`bash`](../tools/builtin/bash.md)。命令还在跑时，工具自己的 `select!` 杀掉子进程并返回 `Err`。循环回到上面的检查点后结束这一轮，不再请求模型 |

前两处检查点都会推 `StreamEvent::Cancelled` 并直接返回，`finish_reason` 标成 `"cancelled"`。bash 自己不推这个事件：它只负责把进程杀掉，循环在下一次检查时收尾。

网络读取过程中的打断不在这里，而在 [`providers/sse.rs`](../providers/sse.md) 的 `tokio::select!` 里。命令还在跑时的打断在 bash 工具里。
