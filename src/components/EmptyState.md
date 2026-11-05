# `EmptyState.tsx`

- 源码：[`EmptyState.tsx`](./EmptyState.tsx)
- 分层：前端展示层

## 职责

会话还没有任何消息时显示的空态：一句标题、一句说明、四张可点击的示例卡片。写法参考 shadcn 的空态范式（标题 + 引导语 + 快捷入口）。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `EXAMPLES` | 四个示例：标题 + 实际的提示词 |
| `EmptyState({ onPick })` | 组件本体，`onPick` 把选中的文字交给外部 |

## 逻辑要点

1. 卡片点一下不是直接发请求，而是把文字填进输入框——用户还能改完再发。
2. 填输入框靠 `store.setDraft`，所以 `EmptyState` 本身不持有状态。

## 依赖关系

- 依赖：无（只接收一个回调）。
- 被谁使用：`components/MessageList.tsx`。

## 当前行为说明

- 示例是写死的四句，换成自己的业务场景直接改 `EXAMPLES` 即可。
