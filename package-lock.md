# `package-lock.json`

- 源码：[`package-lock.json`](./package-lock.json)
- 分层：前端依赖锁文件（**自动生成，勿手改**）

## 职责

锁定 `npm install` 实际安装的每个包的确切版本与完整性哈希。它保证「在我机器上能跑」等价于「在别人机器上也能跑」，是企业级项目必须入库的文件。

## 规则

| 该做 | 不该做 |
| --- | --- |
| 改 [`package.json`](./package.md) 后由 npm 自动更新它，并一起提交 | 手动编辑里面的版本号或哈希 |
| CI 用 `npm ci` 严格按它安装 | 用 `npm install` 后不提交锁文件的变更 |
| 合并冲突时重新 `npm install` 生成 | 逐个手工挑选冲突行 |

## 依赖关系

- 由谁生成：[`package.json`](./package.md) + `npm install`。
- 被谁使用：`npm ci`、CI 缓存键、`npm audit`。

## 当前行为说明

- 文件体积较大（几千行），Code Review 时通常不必逐行读；关注 `packages[""].dependencies` 是否与 `package.json` 一致即可。
