# `tsconfig.json`

- 源码：[`tsconfig.json`](./tsconfig.json)
- 分层：前端 TypeScript 编译配置（应用代码）

## 职责

约束 `src/` 下应用代码的类型检查规则。它不产出 JS（`noEmit: true`），只做检查；真正打包交给 Vite。

## 关键配置

| 配置 | 值 | 作用 |
| --- | --- | --- |
| `target` / `lib` | `ES2020` / `["ES2020", "DOM"]` | 桌面 WebView 支持 ES2020，DOM 类型用于 `window`、`document` |
| `module` / `moduleResolution` | `ESNext` / `bundler` | 交给打包器处理模块解析 |
| `jsx` | `react-jsx` | React 19 的自动 JSX runtime，不用在每个文件写 `import React` |
| `noEmit` | `true` | 只检查、不输出文件 |
| `strict` | `true` | 打开全部严格检查 |
| `noUnusedLocals` / `noUnusedParameters` | `true` | 未使用的变量/参数直接报错——本项目多次靠它发现漏删的 import |
| `noFallthroughCasesInSwitch` | `true` | switch 穿透检查 |
| `isolatedModules` | `true` | 每个文件能独立转译，配合 Vite |
| `include` | `["src"]` | 只管应用代码 |
| `references` | `[{ "path": "./tsconfig.node.json" }]` | 关联构建脚本自己的配置 |

## 依赖关系

- 被谁使用：`npm run build` 里的 `tsc`、编辑器的 TypeScript 语言服务。
- 相关：[`tsconfig.node.md`](./tsconfig.node.md)、[`package.md`](./package.md)。

## 当前行为说明

- `noUnusedLocals: true` 会让调试时临时注释掉的代码报错，这是刻意的：宁可编译失败，也不留死代码。
