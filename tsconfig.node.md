# `tsconfig.node.json`

- 源码：[`tsconfig.node.json`](./tsconfig.node.json)
- 分层：前端 TypeScript 编译配置（构建脚本）

## 职责

单独负责 `vite.config.ts` 的类型检查。它被 `tsconfig.json` 的 `references` 引用，形成「应用代码 / 构建脚本」两套配置。

## 为什么要拆成两份

`vite.config.ts` 跑在 Node 环境里，用的是 Node API（`node:process`）；`src/` 跑在浏览器 WebView 里，用的是 DOM API。两者的类型集合不一样，混在一份配置里会互相污染。

## 关键配置

| 配置 | 值 | 作用 |
| --- | --- | --- |
| `composite` | `true` | 允许被 `references` 引用（要求声明项目间依赖） |
| `include` | `["vite.config.ts"]` | 只覆盖这一个文件 |
| `allowSyntheticDefaultImports` | `true` | 允许 `import process from "node:process"` 这类默认导入写法 |
| `skipLibCheck` | `true` | 跳过第三方声明文件的检查，加快速度 |

## 依赖关系

- 被谁使用：[`tsconfig.json`](./tsconfig.md) 的 `references`、`npm run build` 的 `tsc`。
- 相关：[`vite.config.md`](./vite.config.md)。

## 当前行为说明

- 项目未安装 `@types/node`，所以 `vite.config.ts` 里对 `node:process` 的导入加了 `@ts-expect-error`；如果以后装了 `@types/node`，要同时删掉那行注释，否则 `@ts-expect-error` 会因为「没有错误可压制」而报错。
