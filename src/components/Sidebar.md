# `Sidebar.tsx`

- 源码：[`Sidebar.tsx`](./Sidebar.tsx)
- 分层：前端导航层

## 职责

左侧会话栏：顶部一个「新建对话」按钮，下面是会话列表（按今天 / 昨天 / 更早分组），底部是本地用户和「设置」。布局参考 Chatbox、NextChat、LobeChat 这类桌面 AI 客户端的会话栏。没有登录。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `Sidebar()` | 侧栏组件 |
| `openSession(id)` | 切换会话：先切内存，再从数据库读消息 |
| `remove(id)` | 删除会话：先删数据库，再删内存 |
| `create()` | 新建对话：建完立刻落库 |
| `SettingsPanel` | 底栏上方的小面板：用户、模型、工作区来源、`.env` 里的 `DEEPSEEK_API_KEY` |

## 逻辑要点

1. 只订阅 store 里的 `sessions`、`activeSessionId` 和四个会话动作，不碰 Tauri。
2. `useMemo` 里把会话按 `groupOf(session)` 分桶，桶内保持原顺序（新的在前）。
3. 当前会话那一行加 `sidebar__row--active`，样式上加深底色。
4. 删除按钮默认 `opacity: 0`，鼠标悬停整行时才出现——避免列表里全是叉号。
5. 「新建对话」调 `newSession()`：插入一个空会话并切过去，旧的会话还在列表里；随后立刻 `persistSession` 落库，这样刷新页面它还在。
6. **切换会话的顺序很重要**：先 `selectSession` 切内存，`loadMessagesOf` 才知道往哪个会话里填消息。
7. **删除会话先删数据库**：外键级联会把它的消息一起删掉。
8. 底栏两个按钮都开关同一块设置面板。「本地用户」只是占位，不发起登录。面板里的模型名、工作区、密钥位置都是说明，不能在界面里改。Esc 或「关闭」收起。面板用 `position: absolute` 留在侧栏里，不用 `fixed`，也不加 `transform`。

## 依赖关系

- 依赖：`../store` 的 `useChatStore`、`groupOf`、[`../ipc/sessions`](../ipc/sessions.md) 的三个落库函数。
- 被谁使用：`App.tsx`。

## 当前行为说明

- 会话已落库，刷新页面不会丢。
- 不支持重命名会话；标题自动取第一条用户消息的前 24 个字。
- 工作区根的具体路径不在面板里显示。它来自 `MINI_AGENT_WORKSPACE`，不设则是启动时的当前目录，进程内不再改。
