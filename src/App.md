# `App.tsx`

- 源码：[`App.tsx`](./App.tsx)
- 分层：前端根组件

## 职责

两栏布局的骨架：左边会话栏（[`Sidebar`](./components/Sidebar.md)），右边对话区（顶栏 + 错误条 + 消息列表 + 输入区）。自身不含状态、不含 IPC。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `App()` | 根组件，默认导出 |
| `NARROW_QUERY` | 窄屏断点 `(max-width: 860px)`，必须和 `App.css` 的媒体查询一致 |
| `clearChat()` | 清空当前对话：内存里清，数据库里也清 |

## 逻辑要点

1. `app` 是 flex 容器：侧栏固定 248px，右侧 `main` 占满剩余宽度；`app--collapsed` 把侧栏推到屏幕外（窄屏时是抽屉）。
2. 顶栏只有三样东西：折叠按钮、当前模型名、「清空对话」按钮（没有消息时禁用）。
3. 错误条放在顶栏和消息区之间，点「知道了」调 `clearError`。
4. `sidebarOpen` 的**初始值按窗口宽度算**：宽屏常驻展开，窄屏默认收起。如果无脑设成 `true`，窄屏抽屉一进页面就盖住顶栏、消息和输入框。
5. `useEffect` 里用 `matchMedia(NARROW_QUERY).addEventListener("change", ...)` 跟随窗口变化：变窄自动收起、变宽自动展开。用 `change` 而不是 `resize`，只在跨过断点时触发一次。
5. **启动时 `useEffect` 调 `hydrateFromDb()`**：把数据库里的会话读回来，刷新页面不丢东西。
6. **窄屏时侧栏浮在内容上面**，这时加一层 `.scrim` 遮罩，点它收起侧栏。
7. 「清空对话」要同时清内存和数据库，最后再落库一次把标题重置成「新对话」。
8. 断点写死在两处（JS 常量 + CSS 媒体查询），改数字时**必须两个一起改**，否则 JS 判断的抽屉状态和 CSS 的布局会错位。

## 依赖关系

- 依赖：`./store`、`./components/Sidebar`、`./components/MessageList`、`./components/Composer`、[`./ipc/sessions`](./ipc/sessions.md)、`./App.css`。
- 被谁使用：`main.tsx` 把它挂到 `#root` 上。

## 当前行为说明

- 窗口标题写在 `src-tauri/tauri.conf.json` 的 `productName`（mini_agent），和页面顶部的标题是两处。
- `hydrateFromDb` 的失败被静默忽略：数据库是空的也能正常用。
- 系统缩放（例如 125%）会让 CSS 像素变小，物理宽度看着不小也可能跨过 860px 断点；这是 CSS 像素的正常行为，不是 bug。
