# `package.json`

- 源码：[`package.json`](./package.json)
- 分层：前端依赖与脚本清单

## 职责

声明前端依赖、npm 脚本和项目元信息。`npm install` 依据它和 `package-lock.json` 装依赖。

## 脚本

| 命令 | 实际执行 | 作用 |
| --- | --- | --- |
| `npm run dev` | `vite` | 只起前端开发服务器（1420 端口，无后端） |
| `npm run build` | `tsc && vite build` | **先类型检查再打包**，产物在 `dist/` |
| `npm run preview` | `vite preview` | 本地预览构建产物 |
| `npm run tauri` | `tauri` | 转发给 Tauri CLI，`npm run tauri dev` / `npm run tauri build` |

`build` 里把 `tsc` 放在 `vite build` 前面，是为了让类型错误阻断打包——这是本项目的一道质量门禁。

## 依赖分类

| 类别 | 包 | 用途 |
| --- | --- | --- |
| 运行时框架 | `react`、`react-dom` | UI |
| Tauri 桥 | `@tauri-apps/api`、`@tauri-apps/plugin-opener` | `invoke` / `Channel` / 打开外部链接 |
| 状态 | `zustand` | 全局 store |
| 渲染 | `react-markdown`、`remark-gfm` | 助手消息的 Markdown / 表格 |
| 构建工具 | `vite`、`@vitejs/plugin-react`、`typescript`、`@tauri-apps/cli` | 开发依赖 |

## 当前行为说明

- `dependencies` 里还留有模板自带的 `react-router-dom`、`axios`、`rehype-highlight`、`shiki`、`canvas-confetti` 等**当前未使用**的包。它们是 `create-tauri-app` 模板残留，保留不影响构建，但会增加安装体积；做精简时可以直接移除并重新 `npm install`。
- `version: 0.1.0` 与 `src-tauri/tauri.conf.json` 的 `version`、`Cargo.toml` 的 `version` 是三处，发版时要一起改。
