# `store.ts`

- 源码：[`store.ts`](./store.ts)
- 分层：前端状态层

## 职责

全应用唯一的状态存放处。除了消息列表，还管**多会话**（左侧栏那些条目）、当前会话、生成状态、错误提示和输入框草稿。组件只订阅这里，不认识 Tauri、不认识流式事件。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `UiMessage` | 界面上的一条消息：比后端 `ChatMessage` 多了 `id`、`streaming`、`toolName`、`toolCall`、`createdAt` |
| `Session` | 一个会话：`id`、`title`、`messages`、`createdAt`、`updatedAt` |
| `ChatState` | 状态字段 + 所有动作的类型 |
| `groupOf(session)` | 侧栏分组：今天 / 昨天 / 更早 |
| `activeMessages(state)` | 取当前会话的消息；组件和 ipc 层都用它 |
| `patchActive(state, fn)` | 内部辅助：只替换当前会话，其它会话原样返回 |
| `patchByMessage(state, messageId, fn)` | 内部辅助：按消息 id 反查所属会话并替换它 |
| `useChatStore` | zustand 创建出来的 store |

## 会话相关的动作

| 动作 | 作用 |
| --- | --- |
| `newSession()` | 插入一个空会话并切过去 |
| `selectSession(id)` | 切换当前会话，顺带清掉错误和草稿 |
| `deleteSession(id)` | 删除会话；删的是当前会话时自动切到剩下第一个，全删光则补一个新空会话 |
| `clearActiveSession()` | 清空当前会话里的消息，会话本身留着 |
| `hydrateSessions(metas)` | 用数据库读回来的会话列表替换内存列表（启动时调用） |
| `setSessionMessages(id, messages)` | 用数据库读回来的消息替换某个会话的消息（切换会话时调用） |
| `truncateAfter(id, messageId)` | 删掉某条消息之后的所有消息（重新生成前调用） |

## 消息相关的动作（全部作用在当前会话上）

| 动作 | 作用 |
| --- | --- |
| `appendUser(text)` | 加一条用户消息；如果是本会话第一条消息，用它当标题 |
| `startAssistant()` | 建一条空助手消息，返回它的 id |
| `appendDelta(id, text)` | 给 id 匹配的那一条追加文字 |
| `addToolCards(calls, anchorId)` | 为每个工具调用插一条记录 |
| `finish(anchorId?)` | 结束生成：清掉 `streaming` 标记；一个字都没收到时把空气泡删掉 |
| `fail(message, anchorId?)` | 出错：记录错误、删掉空气泡 |
| `clearError()` / `setDraft(text)` | 杂项 |

## 逻辑要点

1. 每条消息一个 `crypto.randomUUID()` 生成的 `id`；流式追加靠它定位「该往哪一条上写」。
2. 所有会话改动都经过 `patchActive`：只替换当前会话，其它会话原样返回（不可变更新，React 才会正确重渲染）。
3. `EMPTY` 是一个模块级常量数组：作为 selector 的兜底值必须是稳定引用，否则每次渲染都是新数组，React 会认为状态一直在变。
4. `activeMessages(state)` 是组件里订阅消息的入口；`useChatStore((s) => activeMessages(s))` 返回的是会话里那个数组本身，引用稳定。
5. `fail` 会把「空的助手气泡」删掉：出错时用户看到的是错误条，而不是一条空气泡。
6. **`startAssistant` 是 `isStreaming` 唯一的置真点**：停止按钮、发送禁用、重复发送保护全看它。
7. **流式相关动作按「消息 id」定位会话**（`patchByMessage`）：用户在生成过程中切到别的会话时，文字片段必须继续写到原来那条消息上，而不是写进新会话。
8. **`selectSession` 不碰 `isStreaming`**：后端那一轮可能还在跑，把它置 false 会让用户以为没在生成，从而并发发起第二轮。

## 依赖关系

- 依赖：`zustand`、`./types` 的 `ToolCall` / `SessionMeta`。
- 被谁使用：所有组件；`ipc/chat.ts` 通过 `useChatStore.getState()` 在命令式代码里读写状态。

## 当前行为说明

- 会话与消息都会落进 SQLite（见 [`ipc/sessions.md`](./ipc/sessions.md)），但**内存里仍然保留一份**，store 是界面的唯一真相来源。
- 没有做历史裁剪，长对话会把当前会话的全部消息都发给后端。
