# `vite.config.ts`

- 源码：[`vite.config.ts`](./vite.config.ts)
- 分层：前端构建配置

## 职责

配置 Vite 开发服务器与 React 插件。这个文件是 Tauri 模板里为数不多**必须按原样保留**的配置，因为它和 `tauri.conf.json` 的端口约定是一对。

## 关键配置

| 配置 | 值 | 为什么 |
| --- | --- | --- |
| `plugins` | `[react()]` | 支持 JSX 与快速刷新 |
| `clearScreen` | `false` | 不清屏，否则 Rust 编译报错会被 Vite 日志刷掉，看不到 |
| `server.port` | `1420` | 必须等于 `tauri.conf.json` 的 `build.devUrl` 端口 |
| `server.strictPort` | `true` | 端口被占用时**直接失败**而不是偷偷换端口；否则 Tauri 会连到错误的页面 |
| `server.host` | `TAURI_DEV_HOST \|\| false` | 移动/远程调试时由环境变量指定主机 |
| `server.hmr` | 仅当 host 存在时启用，ws / 1421 | 远程开发时的热更新通道 |
| `watch.ignored` | `["**/src-tauri/**"]` | 忽略 Rust 目录，避免 `target/` 里海量文件触发无意义的热更新 |

## 依赖关系

- 依赖：`vite`、`@vitejs/plugin-react`、`node:process`（用 `@ts-expect-error` 绕过缺少 `@types/node` 的报错）。
- 与谁配对：[`src-tauri/tauri.conf.json`](./src-tauri/tauri.conf.md) 的 `devUrl` / `beforeDevCommand` / `frontendDist`。

## 当前行为说明

- 报 `Port 1420 is already in use` 说明已有一个 `tauri dev` 在跑，关掉旧窗口或结束旧进程即可，不要改端口。
- 第 3 行的 `@ts-expect-error` 是模板写法：项目没装 `@types/node`，但 `vite.config.ts` 由 `tsconfig.node.json` 单独检查，所以这里显式压制类型报错。
