# `ipc/chat.ts`

- 源码：[`chat.ts`](./chat.ts)
- 分层：前端 IPC 层

## 职责

全前端唯一认识 Tauri 的文件：调用后端命令，把后端推来的 `StreamEvent` 翻译成 store 动作。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `friendlyError(error)` | 把缺密钥、401、网络错误收成短中文；认不出来就用原文 |
| `DEFAULT_MODEL` | 默认模型名 `deepseek-chat`（后端目前固定 DeepSeek） |
| `toHistory(messages)` | 界面消息 → 发给后端的对话历史 |
| `runAgent(history, sessionId)` | 跑一轮 agent：建通道、发请求、翻译事件、收尾落库 |
| `sendMessage(text)` | 发一条用户消息，跑完整 agent 流程 |
| `stopGeneration()` | 调后端 `stop_generation`，打断正在进行的网络读取 |
| `regenerate()` | 删掉最后一条助手回复，用同样的历史再问一次 |

## 逻辑要点

1. 生成中直接 `return`，避免重复发送。
2. 先 `appendUser` 再用 `activeMessages` 取历史——这样新消息也在历史里，而且取的是**当前会话**的历史。
3. `startAssistant()` 拿到 id，之后每个文字片段都追加到这一条上。
4. `new Channel<StreamEvent>(回调)` 必须在 `invoke` 之前设好回调，否则早期事件会丢。
5. 事件翻译：`Done` → `finish`；`Cancelled` → `markStopped`（保留半截文字）；`TextDelta` → `appendDelta`；`ToolCalls` → `addToolCards`；`ToolResult` → `completeTool`。
6. **每个事件处理都带一个 `assistantId`**：它是这一轮那条助手消息的 id。后端推事件时前端可能已经切到别的会话了，靠这个 id 反查消息属于哪个会话，文字才不会写错地方。
7. **`runAgent` 是发送和重新生成的公共路径**：抽出来是为了让「重新生成」不会漏掉收尾和落库。
8. `invoke("chat", { req, onEvent })`：Rust 的 `on_event` 在前端叫 `onEvent`（Tauri 2 参数默认 camelCase）；`state` 由框架注入，前端不用传。
9. `invoke` 的 resolve 表示这一趟结束，所以除了后端推来的 `Done`，这里还要再 `finish` 一次兜底；reject 走 `fail`。
10. `finally` 里调 `persistSession(sessionId)`：**成功、失败、被取消都要落库**，这样界面上能看到的对话都存下来了。

## 重新生成为什么不能走 sendMessage

`sendMessage` 会先调 `appendUser` 追加一条用户消息。而重新生成时，那条用户消息**还在**（截断只删掉了它后面的助手回复），再追加一次就多出一条重复的提问。

所以 `regenerate` 直接调 `runAgent`，不经过 `sendMessage`。

## 为什么历史里只留 user / assistant

后端每一轮的 tool 消息都带 `tool_call_id`，前端手里没有这些 id，硬凑会配对失败。所以前端只发纯文本对话，工具消息由后端在自己那一轮内部维护。

## 依赖关系

- 依赖：`@tauri-apps/api/core` 的 `Channel` 与 `invoke`、`../types`、`../store`、[`./sessions`](./sessions.md) 的落库与截断。
- 被谁使用：`components/Composer.tsx`、`components/MessageList.tsx`（重新生成按钮）。

## 当前行为说明

- `tools` 不传，由后端注入自己的工具清单。
- 请求失败都会变成顶部错误条。缺 `DEEPSEEK_API_KEY`、401、网络错误先经过 `friendlyError`，其它错误（比如轮数超限）仍显示原文。
- 发送和重新生成都先 `beginTurn()`。重新生成要等数据库截断，截断失败只收回停止按钮并显示错误，不再发请求。这段等待里点停止会 `noteStop()`：截断结束后不再调用 `chat`。后端那一轮已经开始之后，仍靠取消令牌和 `Cancelled`。
- 「停止生成」需要后端配合：前端调 `stop_generation`，后端的取消令牌触发，正在读网络流的地方会优雅退出并推 `Cancelled` 事件。
