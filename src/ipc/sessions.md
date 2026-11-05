# `sessions.ts`

- 源码：[`sessions.ts`](./sessions.ts)
- 分层：前端 IPC 层 / 持久化

## 职责

前端和 SQLite 之间的桥：启动时把会话读回来、每轮结束后把消息写回去、切换/删除/清空时同步数据库。

## 为什么单独一个文件

`ipc/chat.ts` 负责「流式聊天」，`ipc/sessions.ts` 负责「落库」。两个文件都只碰后端命令，组件永远不直接 `invoke`。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `toStored(message)` | 界面消息 → 能落库的形状 |
| `toUi(message)` | 数据库里的消息 → 界面消息 |
| `hydrateFromDb()` | 启动时读回全部会话，并把当前会话的消息也读出来 |
| `loadMessagesOf(sessionId)` | 读某个会话的全部消息，填进 store |
| `persistSession(sessionId)` | 把某个会话的元信息 + 全部消息写回数据库 |
| `removeSession(sessionId)` | 删除一个会话 |
| `clearMessages(sessionId)` | 清空某个会话的消息，保留会话本身 |
| `truncateAfter(sessionId, messageId)` | 删掉某条消息之后的所有消息（重新生成时用） |

## 逻辑要点

1. **参数名是驼峰**：Rust 侧是 `session_id` / `after_id`，前端要写 `sessionId` / `afterId`（Tauri 2 参数默认 camelCase）。
2. **消息 id 沿用界面上的 id**：这样重新落库时是 `INSERT OR REPLACE` 覆盖，而不是新增重复行。
3. **`seq` 用数组下标**：正好就是对话顺序，读取时 `ORDER BY seq ASC` 就是正确顺序。
4. **`persistSession` 会重写全部消息**：因为存储层用 `INSERT OR REPLACE` + 消息 id 主键，重复写是安全的。
5. **数据库里一个会话都没有时**，`hydrateFromDb` 会把内存里那个空会话落库，界面上就有东西了。
6. **`truncateAfter` 要在重新生成前调用**：先删内存里的（界面立刻更新），再删数据库里的，两边保持一致。
7. **`toUi` 不读状态列**（表里也没有）。工具消息：正文以「工具执行失败」开头则 `toolStatus` 为 `error`，否则为 `done`。助手消息的 `phase` 不从库里恢复，重新打开就是一条普通回复。

## 依赖关系

- 依赖：`@tauri-apps/api/core` 的 `invoke`、`../types`、`../store`。
- 被谁使用：[`ipc/chat.ts`](./chat.md)（每轮结束落库）、[`components/Sidebar.tsx`](../components/Sidebar.md)（切换/新建/删除）、[`App.tsx`](../App.md)（启动 hydrate、清空）。

## 当前行为说明

- 每次落库是逐条 `append_message`，没有批量事务；消息多时会发多次 IPC。
- 没有做写入去重或版本号，理论上「落库中途崩溃」会留下半截对话（下次启动能读到已落库的那部分）。
