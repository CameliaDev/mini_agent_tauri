# `vite-env.d.ts`

- 源码：[`vite-env.d.ts`](./vite-env.d.ts)
- 分层：前端类型声明（Vite 环境）

## 职责

只有一行三斜杠指令，把 Vite 提供的客户端类型引入项目：

```ts
/// <reference types="vite/client" />
```

## 它带来了什么

| 能力 | 例子 |
| --- | --- |
| `import.meta.env` 的类型 | `import.meta.env.DEV` / `MODE` 等 |
| 静态资源导入的类型 | `import logo from "./x.svg"` 不会报「找不到模块」 |
| `*.css` 模块声明 | `import "./App.css"` 能通过类型检查 |

## 依赖关系

- 被谁使用：[`tsconfig.md`](../tsconfig.md) 的 `include: ["src"]` 会自动纳入这个文件。
- 相关：[`main.md`](./main.md)、[`App.md`](./App.md)（都 import 了非 TS 资源或依赖 DOM 类型）。

## 当前行为说明

- 这是 Vite 模板自带文件，**不要删**：删掉后 `import "./App.css"` 之类会类型报错。
- 文件里只有类型声明，不产生任何运行时代码。
