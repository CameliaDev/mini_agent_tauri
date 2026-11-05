# `tauri.conf.json`

- 源码：[`tauri.conf.json`](./tauri.conf.json)
- 分层：Tauri 应用配置（窗口、构建、打包）

## 职责

把「前端怎么起」「窗口长什么样」「安装包怎么打」三件事写在一处。Tauri CLI 和构建脚本都读它。

## 分段说明

| 段 | 字段 | 值 / 作用 |
| --- | --- | --- |
| `build` | `beforeDevCommand` | `npm run dev`：`tauri dev` 先起 Vite |
| | `devUrl` | `http://localhost:1420`，**必须等于** [`vite.config.ts`](../vite.config.md) 的 `server.port` |
| | `beforeBuildCommand` | `npm run build`：打包前先构建前端 |
| | `frontendDist` | `../dist`：前端产物目录 |
| `app` | `windows[0]` | 标题 `mini_agent`、初始 `800×600`；窗口可自由缩放，所以前端必须做响应式（见 [`App.css.md`](../src/App.css.md)） |
| | `security.csp` | `null`，即不启用 CSP |
| `bundle` | `active` / `targets` | 开启打包、生成所有平台格式 |
| | `icon` | 图标列表，`icons/` 目录下的 PNG / ICO / ICNS |

## 与前端的两个契约

1. **端口必须一致**：`devUrl` 的 1420 ↔ Vite 的 `server.port`。任一处改了另一处没改，窗口会白屏。
2. **窗口尺寸会变**：初始 800×600 高于前端的抽屉断点（≤640 CSS px），所以侧栏默认展开，和主栏并排，不盖住对话。再缩到 640px 及以下才变成抽屉并默认收起。断点在 [`App.tsx`](../src/App.md) 的 `NARROW_QUERY`。

## 依赖关系

- 依赖：[`vite.config.md`](../vite.config.md)、[`capabilities/default.md`](./capabilities/default.md)。
- 被谁使用：Tauri CLI、`build.rs`、`lib.rs` 的 `generate_context!()`。

## 当前行为说明

- `identifier` 仍是模板默认值 `com.tauri-app.mini_agent`，正式发布前要改成自己的域名反写。
- `productName` / `version` 与 `package.json`、`Cargo.toml` 是三处。
- `security.csp: null` 表示未启用内容安全策略；本项目不渲染用户提供的 HTML（`react-markdown` 默认转义），风险较低，但对标企业级时应配一份严格 CSP。
