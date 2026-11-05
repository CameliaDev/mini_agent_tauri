# `MessageList.tsx`

- 源码：[`MessageList.tsx`](./MessageList.tsx)
- 分层：前端展示层

## 职责

消息滚动区。三种内容分三种排版：助手消息无气泡（左侧小头像 + 正文）、用户消息右对齐浅色气泡、工具调用做成一张可折叠的卡片。分区写法参考 shadcn 的 Message / Bubble。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `MessageList()` | 组件本体 |
| `ToolCard` | 工具名 + 运行中/完成/失败/已停止；有输出时默认折叠 |

## 逻辑要点

1. 订阅 `activeMessages(s)` 拿到当前会话的消息；会话切换时列表自动换成新会话的内容。
2. **滚动用 [`useStickToBottom`](../hooks/useStickToBottom.md) 锚定**，不是朴素的 `scrollIntoView`：用户往上翻时不会被硬拽回底部；不贴底时右下角出现「回到底部」按钮。
3. 最后一条助手回复下面有「重新生成」按钮（鼠标悬停才显示），点了会删掉旧回复用同样的历史再问一次。
4. 助手消息用 `react-markdown` + `remark-gfm` 渲染正文，段落/列表/代码块/表格都在 `.msg__body` 作用域内排版。
5. 工具调用不进气泡，而是 `.toolcard`：名字、状态，结束后若有输出可以展开。默认折叠。运行中没有展开按钮。
6. 助手气泡上的阶段：还没收到第一个字符显示「思考中…」（只做透明度变化，不用 `background-clip: text`，否则 Linux 上输入框会跑到屏幕左上角，见 [#7](https://github.com/CameliaDev/mini_agent_tauri/issues/7)）；正在输出时末尾跟闪烁光标，并标「生成中」；用户停止标「已停止」并保留半截文字；失败标「生成失败」。

## 依赖关系

- 依赖：`react-markdown`、`remark-gfm`、`../store`、`./EmptyState`、[`../hooks/useStickToBottom`](../hooks/useStickToBottom.md)、`../ipc/chat` 的 `regenerate`。
- 被谁使用：`App.tsx`。

## 当前行为说明

- 助手消息里的 HTML 不会被渲染（`react-markdown` 默认转义），模型输出里的 `<script>` 只会变成文本。
- 工具卡片的输出来自 `StreamEvent::ToolResult`。历史消息没有状态列，读回来时按正文是否以「工具执行失败」开头区分失败和完成。空输出不提供展开。
- 「重新生成」只对**最后一条**助手回复显示；它前面的助手回复没有这个按钮。
