# `useSidebarWidth.ts`

- 源码：[`useSidebarWidth.ts`](./useSidebarWidth.ts)
- 分层：前端通用 Hook

## 职责

并排布局下，让会话栏可以拖右缘改宽度，并把用户选的宽度记在 `localStorage`。抽屉模式不读这个宽度。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `SIDEBAR_DEFAULT_WIDTH` | 没拖过时的宽度，248px |
| `SIDEBAR_MIN_WIDTH` / `SIDEBAR_MAX_WIDTH` | 绝对上下限，180 / 400 |
| `MAIN_MIN_WIDTH` | 并排时主栏至少留 400px |
| `SIDEBAR_WIDTH_STORAGE_KEY` | `mini_agent.sidebarWidth` |
| `clampPreferredSidebarWidth` | 把用户的选择夹进 180–400，不看窗口 |
| `clampSidebarWidth` | 再按当前窗口收一刀，保证主栏够宽 |
| `useSidebarWidth(appRef)` | 读宽度、写 CSS 变量、处理拖动和方向键 |

## 逻辑要点

1. 用户选的宽度和画面上的宽度是两回事。窗口暂时变窄时画面收一截，**不改 localStorage**，变宽后再恢复。只有真正拖了或按了方向键才写入。
2. 宽度写在 `.app` 的 `--sidebar-width` 上。拖动过程中直接改这个变量，不 `setState`，避免每一格指针移动都重绘消息列表。
3. 收起时如果宽度要变（窗口变窄触发的那一刀），先加上 `app--no-sidebar-transition` 再改。宽度没有过渡、负 margin 有，两者错开会把主栏拽出屏幕。
4. 只点一下、指针没移动，不算一次调整，避免把「暂时收窄」的宽度写进去。
5. 方向键：左右（上下同效）一步 8px，按住 Shift 一步 24px；Home / End 到当前允许的最小 / 最大。

## 依赖关系

- 依赖：React，以及 `App.css` 里的 `--sidebar-width`。
- 被谁使用：`App.tsx` 的拖动手柄。

## 当前行为说明

- 键名是 `mini_agent.sidebarWidth`，值是像素数字的字符串。读到非法内容就退回 248。
- 写不进 `localStorage`（隐私模式等）时，这次会话里仍然能拖，刷新后回到默认。
- ≤640px 的抽屉宽度由 CSS 固定，这个 hook 改变量也不会把抽屉拉宽。
