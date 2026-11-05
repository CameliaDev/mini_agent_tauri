# 前端总览

本目录是 Tauri 应用的前端（React + TypeScript + Vite），后端在 `../src-tauri`。

## 一句话主线

**用户输入 → store 里加一条消息 → ipc 层 invoke 后端 → 后端推流式事件 → ipc 层翻译成消息 → store 更新 → 组件重绘。**

## 界面结构

布局参考 Chatbox / NextChat / LobeChat 这类桌面 AI 客户端：**左边会话栏 + 右边对话区**；消息排版参考 shadcn 的 `Message` / `Bubble`（助手无气泡、用户浅气泡、工具调用用可折叠卡片）。

```
┌────────────┬──────────────────────────────────┐
│ 会话栏      │ 顶栏：模型名 / 清空对话            │
│ 新建对话    ├──────────────────────────────────┤
│ 今天        │                                  │
│  · 会话 A   │  消息区（可滚动，两端渐隐）        │
│  · 会话 B   │                                  │
│ 昨天        │                                  │
│ 更早        ├──────────────────────────────────┤
│ 本地用户 设置│ 输入区（居中卡片）                │
└────────────┴──────────────────────────────────┘
```

## 分层

| 文件 | 职责 | 认识 Tauri 吗 |
| --- | --- | --- |
| [`types.ts`](./types.md) | 前后端共用的数据结构 | 否 |
| [`store.ts`](./store.md) | 唯一的状态仓库（zustand）：会话 + 消息 + 生成状态 | 否 |
| [`ipc/chat.ts`](./ipc/chat.md) | 调用后端、把事件翻译成消息 | **是** |
| [`ipc/sessions.ts`](./ipc/sessions.md) | 会话与消息的落库 | **是** |
| [`hooks/useStickToBottom.ts`](./hooks/useStickToBottom.md) | 流式滚动锚定 | 否 |
| [`components/Sidebar.tsx`](./components/Sidebar.md) | 左侧会话栏 | 否 |
| [`components/MessageList.tsx`](./components/MessageList.md) | 消息滚动区 | 否 |
| [`components/EmptyState.tsx`](./components/EmptyState.md) | 空态与示例卡片 | 否 |
| [`components/Composer.tsx`](./components/Composer.md) | 底部输入区 | 否 |
| [`App.tsx`](./App.md) | 两栏骨架 | 否 |
| [`App.css`](./App.css.md) | 全部样式 | 否 |

规则只有一条：**除了 `ipc/` 目录，任何组件都不许 import `@tauri-apps/api`**。这样后端协议变了，前端只改 `ipc/` 里的文件。

## 与后端的对应关系

| 前端 | 后端 |
| --- | --- |
| `invoke("chat", { req, onEvent })` | `#[tauri::command] chat(state, on_event, req)`（`app/commands.rs`） |
| `invoke("stop_generation")` | `#[tauri::command] stop_generation(state)`（`app/commands.rs`） |
| `invoke("list_sessions")` 等 | `app/commands.rs` 里那组会话持久化命令 |
| `onEvent: Channel<StreamEvent>` | `tauri::ipc::Channel<StreamEvent>` |
| `StreamEvent` 的三种形态 | `agent/events.rs` 里的 `StreamEvent` 枚举 |
| `ChatRequest` / `ChatResponse` | `protocol.rs` 里的同名结构 |

两个容易踩的点：

- Tauri 2 的命令参数默认 **camelCase**，所以 Rust 的 `on_event` 在前端要写 `onEvent`；
- `state` 是框架注入的，前端不用传。

## 一次发送的数据流

1. `Composer` 调 `sendMessage(text)`（`ipc/chat.ts`）
2. `store.appendUser` 在当前会话里加一条用户消息（并用它当会话标题）
3. `store.startAssistant` 建一条空助手消息，拿到它的 id
4. `new Channel(...)` 注册回调，把事件翻译成 store 动作
5. `invoke("chat", { req, onEvent })`
6. 后端推 `TextDelta` → `appendDelta`；推 `ToolCalls` → `addToolCards`；推 `ToolResult` → `completeTool`；推 `Done` → `finish`；推 `Cancelled` → `markStopped`（保留半截文字，标成已停止）
7. `invoke` resolve / reject → `finish` / `fail` 兜底关掉「生成中」
8. `finally` → `persistSession(sessionId)` 把这一轮落库

## 数据落在哪

```
内存（zustand store）──界面读这里──┐
                                   │ 每轮结束 / 切换 / 删除时同步
                    SQLite（mini_agent.db）
                    sessions 表 + messages 表
```

- 库文件放在系统应用数据目录（Windows 是 `%APPDATA%/<应用名>/mini_agent.db`）；
- 刷新页面、重启程序都不会丢；
- 删会话时消息随外键级联一起删。

## 自己写的时候

先在 `App.tsx` 里用 `useState` 把消息数组画出来（不拆文件），跑通之后再抽成组件 —— 组件化是同一段代码写第二遍时抽出来的，不是开工第一天就该建的目录。

持久化也建议分两步：先做「刷新后还在」的最简版本（每轮结束把整个会话写进一个 JSON 文件），跑通了再换成 SQLite——这样出问题时报错信息更少。
