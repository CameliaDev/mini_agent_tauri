# `ci.yml`

- 源码：[`ci.yml`](./ci.yml)
- 分层：仓库质量门（GitHub Actions）

## 职责

每个 PR 和 `main` 上的推送跑两件事：Rust 的格式、lint、单元测试；前端的依赖安装、类型检查、Vite 构建。不打包 Tauri 安装包。

## 包含的定义

| Job | 做什么 |
| --- | --- |
| `rust` | 在 `src-tauri/` 里跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` |
| `frontend` | `npm ci`、`npx tsc --noEmit`、`npm run build` |

触发：`pull_request`，以及推送到 `main`。同一分支的新提交会取消还在跑的旧任务。`permissions` 只有 `contents: read`。

## 逻辑要点

1. **Rust 用 stable**，并装上 `rustfmt` 和 `clippy`。版本不钉死，格式化以当前 stable 的 rustfmt 为准。
2. **先装 Linux 系统库再编译**。Tauri 2 即使只跑 `cargo test` 也要链接 WebKit。脚本优先找 `libwebkit2gtk-4.1-dev`，没有再退到 `4.0`，避免 Ubuntu 升级后包名一对不上就红。
3. **前端用 Node 22**。README 写的是 Node 20+，22 满足这条。`npm ci` 吃锁文件，不重新解析依赖范围。
4. **`npm run build` 是 `tsc && vite build`**。前面的 `tsc --noEmit` 会和它重复一次类型检查，这是刻意的：类型错误和打包错误在日志里分开。
5. **不跑 `tauri build`**。安装包要系统 WebView 和更重的打包步骤，质量门用类型检查加单测就够。

## 依赖关系

- 与谁配对：[`package.json`](../../package.md) 的 `build` 脚本、[`src-tauri/Cargo.toml`](../../src-tauri/Cargo.md)、[`docs/handcraft.md`](../../docs/handcraft.md) 的完成检查清单。
- 被谁使用：GitHub 在 PR 上自动跑。本地对应的命令写在 [`README.md`](../../README.md) 的质量门一节。

## 当前行为说明

- 缓存用 `Swatinem/rust-cache`，只影响速度，不改变检查内容。
- `cargo test` 不需要 API Key，也不读仓库里的 `.env`。
- 工作流失败时看对应 job 的日志即可，没有额外的状态上报。
