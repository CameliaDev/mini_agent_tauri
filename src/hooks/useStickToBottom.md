# `useStickToBottom.ts`

- 源码：[`useStickToBottom.ts`](./useStickToBottom.ts)
- 分层：前端通用 Hook

## 职责

让消息区在「用户想看底部」时自动跟随流式输出，在「用户往上翻」时别抢他的滚动位置。

## 问题是什么

流式输出时每个字都会追加到消息末尾。如果无脑 `scrollTop = scrollHeight`，用户想回头看之前的内容时会被硬生生拽回底部，很烦。

## 做法（业界通行方案）

ChatGPT、shadcn-chatbot-kit 都是这个思路：

1. 监听容器的 `scroll` 事件，算出「离底部还有多少像素」；
2. 距离小于阈值（`STICK_THRESHOLD = 80`）就认为用户想看底部 → `isPinned = true`；
3. 距离大于阈值说明用户往上翻了 → `isPinned = false`；
4. 内容变化时，**只有 `isPinned` 为 true 才自动滚到底**；
5. 用户往上翻时，界面右下角出现一个「回到底部」按钮。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `const STICK_THRESHOLD` | 阈值：离底部小于 80px 就认为贴着底部 |
| `interface StickToBottom` | 返回值类型 |
| `function useStickToBottom(content)` | 钩子本体 |

## 返回值

| 字段 | 作用 |
| --- | --- |
| `scrollRef` | 挂到消息滚动容器上的 ref |
| `isPinned` | 用户是否贴着底部（决定要不要显示「回到底部」按钮） |
| `scrollToBottom()` | 手动滚到底（点按钮时用） |
| `onScroll()` | 滚动事件的处理函数，挂在容器的 `onScroll` 上 |

## 逻辑要点

1. **`pinnedRef` 和 `isPinned` 双份状态**：scroll 回调里要读到最新值，但把 `isPinned` 放进 `useEffect` 依赖会让回调每次重建，得不偿失。所以用 ref 存「真相」，用 state 触发重渲染。
2. **内容变化时用 `el.scrollTop = el.scrollHeight` 而不是 `scrollTo({behavior:"smooth"})`**：流式输出时每个字都会触发一次，smooth 反而会一直排队动画、看起来卡顿。
3. **依赖值由调用方传入**：`MessageList` 传的是「消息条数-末条长度-是否在生成」，这样流式文字变长就能触发跟随。
4. **首次挂载时延后一帧滚到底**：等 DOM 真正渲染出高度再滚，否则 `scrollHeight` 还是 0。

## 依赖关系

- 依赖：`react` 的 `useCallback` / `useEffect` / `useRef` / `useState`。
- 被谁使用：[`components/MessageList.tsx`](../components/MessageList.md)。

## 当前行为说明

- 只处理「贴底 / 不贴底」两种状态，没有做「用户正在拖动时暂停跟随」这种更细的判断。
- 阈值 80px 是拍脑袋的经验值，滚动条粗一点的话可能要调大。
