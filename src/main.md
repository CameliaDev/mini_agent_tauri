# `main.tsx`

- 源码：[`main.tsx`](./main.tsx)
- 分层：前端入口

## 职责

React 挂载入口：把 `App` 渲染到 `index.html` 里的 `#root` 节点上。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `ReactDOM.createRoot(...).render(...)` | 创建根节点并渲染 `<App />`，外层套一层 `React.StrictMode` |

## 依赖关系

- 依赖：`react-dom/client`、`./App`。
- 被谁使用：`index.html` 里的 `<script type="module" src="/src/main.tsx">`。

## 当前行为说明

- `document.getElementById("root")` 用了 `as HTMLElement` 断言，因为 TypeScript 不知道元素一定存在；模板里 `#root` 是写死的。
- `StrictMode` 在开发环境会故意把组件渲染两次、副作用跑两次，用来暴露副作用写错的地方；正式构建不会有这个行为。
