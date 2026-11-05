# `store.ts`

- 源码：[`store.ts`](./store.ts)
- 分层：前端状态层

## 职责

全应用唯一的状态存放处。除了消息列表，还管**多会话**（左侧栏那些条目）、当前会话、生成状态、错误提示和输入框草稿。组件只订阅这里，不认识 Tauri、不认识流式事件。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `UiMessage` | 界面上的一条消息：比后端 `ChatMessage` 多了 `id`、`streaming`、`phase`、`toolName`、`toolCall`、`toolStatus`、`createdAt` |
| `AssistantPhase` / `ToolStatus` | 助手气泡的阶段，以及工具卡片的状态。不落库 |
| `Session` | 一个会话：`id`、`title`、`messages`、`createdAt`、`updatedAt`、`messagesLoaded` |
| `applyIncomingMessages(session, incoming)` | 把数据库消息灌进还没接管的会话。已接管的原样返回；加载期间刚发出的消息留在末尾 |
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
| `setSessionMessages(id, messages)` | 首次把数据库消息灌进该会话。`messagesLoaded` 已经为 true 时不盖掉内存 |
| `truncateAfter(id, messageId)` | 删掉某条消息之后的所有消息（重新生成前调用） |

## 消息相关的动作（全部作用在当前会话上）

| 动作 | 作用 |
| --- | --- |
| `appendUser(text)` | 加一条用户消息；如果是本会话第一条消息，用它当标题 |
| `beginTurn()` | 把 `isStreaming` 置真、清掉错误和上一轮的停止标记。发送和重新生成一进来就调 |
| `noteStop()` | 本地记「用户已经点了停止」。请求还没进后端时，`runAgent` 看到它就不再发出去 |
| `startAssistant()` | 建一条空助手消息，`phase` 为 `waiting`，返回它的 id |
| `appendDelta(id, text)` | 给 id 匹配的那一条追加文字，并把 `phase` 改成 `streaming` |
| `addToolCards(calls, anchorId)` | 为每个工具调用插一条 `toolStatus: running` 的记录 |
| `completeTool(callId, output, isError)` | 按 `toolCall.id` 写上输出，状态改成完成或失败 |
| `finish(anchorId?)` | 正常结束：清掉 `streaming`。还停在 `waiting` 且没有文字的气泡删掉。已经是停止或失败的不要改掉 |
| `markStopped(anchorId?)` | 用户停止：保留文字（包括空的），气泡标成 `stopped`，还在跑的工具标成 `stopped` |
| `fail(message, anchorId?)` | 出错：顶部记一条错误，这一轮助手气泡标成 `failed`，半截文字留着 |
| `clearError()` / `setDraft(text)` | 杂项 |

## 逻辑要点

1. 每条消息一个 `crypto.randomUUID()` 生成的 `id`；流式追加靠它定位「该往哪一条上写」。
2. 所有会话改动都经过 `patchActive`：只替换当前会话，其它会话原样返回（不可变更新，React 才会正确重渲染）。
3. `EMPTY` 是一个模块级常量数组：作为 selector 的兜底值必须是稳定引用，否则每次渲染都是新数组，React 会认为状态一直在变。
4. `activeMessages(state)` 是组件里订阅消息的入口；`useChatStore((s) => activeMessages(s))` 返回的是会话里那个数组本身，引用稳定。
5. `fail` 保留助手气泡并标成失败，同时在顶部放错误条。`markStopped` 同样留着气泡，用来显示「已停止」。
6. **`beginTurn` 和 `startAssistant` 都会把 `isStreaming` 置真**。发送、重新生成先走 `beginTurn`，这样数据库截断还没返回时停止按钮也已经换上。`startAssistant` 再置一次，保证气泡和按钮一致。
7. **流式相关动作按「消息 id」定位会话**（`patchByMessage`）：用户在生成过程中切到别的会话时，文字片段必须继续写到原来那条消息上，而不是写进新会话。
8. **`selectSession` 不碰 `isStreaming`**：后端那一轮可能还在跑，把它置 false 会让用户以为没在生成，从而并发发起第二轮。
9. **`messagesLoaded`**：本地新建、清空、截断，或第一次成功灌库之后为 true。之后再读到的数据库快照不能替换 `messages`。发送后立刻切走再切回来时，库里往往还没有这一轮，不能拿空列表把内存盖掉。

## 依赖关系

- 依赖：`zustand`、`./types` 的 `ToolCall` / `SessionMeta`。
- 被谁使用：所有组件；`ipc/chat.ts` 通过 `useChatStore.getState()` 在命令式代码里读写状态。

## 当前行为说明

- 会话与消息都会落进 SQLite（见 [`ipc/sessions.md`](./ipc/sessions.md)），但**内存里仍然保留一份**，store 是界面的唯一真相来源。
- 没有做历史裁剪，长对话会把当前会话的全部消息都发给后端。
