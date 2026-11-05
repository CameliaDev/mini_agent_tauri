# `App.css`

- 源码：[`App.css`](./App.css)
- 分层：前端样式

## 职责

整个界面的样式。两栏骨架与会话栏参考 Chatbox / NextChat / LobeChat，消息排版参考 shadcn 的 Message / Bubble / Marker。组件里只用语义化类名，外观全写在这里。

## 结构速览

| 一块 | 类名 |
| --- | --- |
| 配色 | `:root` 里的 CSS 变量；深色模式在 `@media (prefers-color-scheme: dark)` 里覆盖同一批变量 |
| 两栏骨架 | `.app`、`.app--collapsed`、`.main` |
| 会话栏 | `.sidebar`、`.sidebar__new`、`.sidebar__group-title`、`.sidebar__row`、`.sidebar__item`、`.sidebar__del`、`.sidebar__foot` |
| 顶栏 | `.topbar`、`.topbar__toggle`、`.topbar__meta`、`.topbar__action` |
| 错误条 | `.banner`、`.banner__text` |
| 消息区 | `.messages-wrap`（上下渐隐）、`.messages`、`.msg`、`.msg__avatar`、`.msg__body`、`.bubble--user` |
| 工具标记 | `.marker`、`.marker__badge`、`.marker__name`、`.marker__args` |
| 流式提示 | `.shimmer`（微光文字）、`.caret`（闪烁光标） |
| 空态 | `.empty`、`.empty__grid`、`.empty__card` |
| 输入区 | `.composer`、`.composer__box`、`.composer__input`、`.composer__foot`、`.composer__actions`、`.composer__send`、`.composer__stop` |
| 消息操作 | `.msg__actions`、`.msg__action`（悬停才显示的「重新生成」） |
| 回到底部 | `.jump-bottom`（不贴底时才出现） |
| 窄屏遮罩 | `.scrim` |

## 几个关键手法

- **两栏**：`.app` 是 flex，侧栏固定 248px；`app--collapsed` 用 `margin-left: -248px` 把它推出屏幕（窄屏时它就是抽屉）。
- **滚动渐隐**：`.messages-wrap::before/::after` 做上下渐变遮罩，提示上面还有内容，不用额外的滚动监听。
- **助手无气泡**：正文直接排在页面底色上，只有用户消息用 `.bubble--user` 浅灰气泡——这是成熟 AI 客户端常见的做法，长回答更好读。
- **工具用 Marker**：`.marker` 左边一道竖线 + 小标签，表示「这是一条系统动作」，不混进气泡。
- **颜色只有一处来源**：所有颜色都是变量，深色模式只覆盖变量，规则不用改。

## 两个布局 bug 的修法（值得学）

### 1. 内容横向溢出窗口

flex 子项默认 `min-width: auto`，意思是「我不小于内容宽度」。消息区里的 markdown 代码块 / 长表格会把容器撑破。

修法：给 `.messages-wrap` 加 `min-width: 0`，让它可以收缩，横向滚动交给内部的 `.messages`。

### 2. 窄屏下侧栏盖住主区

侧栏在窄屏时是 `position: absolute` 的抽屉，**默认展开就会盖住内容**。要同时做对三件事：

1. `.app` 加 `position: relative`，给 absolute 抽屉一个明确的定位基准，并给抽屉补 `top: 0; left: 0`；
2. 抽屉收起用 `transform: translateX(-100%)`，**不要用 `margin-left`** —— absolute 元素已经脱离 flex 流，margin 不会让主区补上这块宽度；
3. `.scrim` 遮罩点一下收起侧栏。

### 3. CSS 顺序坑：遮罩永远不显示

`.scrim` 的基础规则是 `display: none`。如果"窄屏改成 `display: block`"这条媒体查询**写在基础规则之前**，同优先级下后写的基础规则会赢，遮罩就永远出不来。

**规则**：覆盖规则必须写在被覆盖规则之后。同理，往样式表末尾 `cat >>` 追加新样式前，先看一眼文件里已有的媒体查询位置。

## 依赖关系

- 被谁使用：`App.tsx` 里 `import "./App.css"`，样式表由入口组件引入，全局生效。

## 当前行为说明

- 窄屏（≤860px）下侧栏变成抽屉，**默认收起**（由 `App.tsx` 的 `matchMedia` 决定），需要点顶栏的 ☰ 展开；空态示例卡片在窄屏改成单列。
- 侧栏的 `transition` 同时覆盖 `margin-left`（宽屏收起）和 `transform`（窄屏抽屉）两种动画。
