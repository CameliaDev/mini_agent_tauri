# `cancel.rs`

- 源码：[`cancel.rs`](./cancel.rs)
- 分层：agent 层 / 取消机制

## 职责

让「用户点了停止生成」这件事，能传进正在跑的网络读取里。

没有它的时候，`chat` 命令一旦跑起来就只能等模型说完；用户关掉页面时，模型还在继续生成，白花钱。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `struct CancelToken` | 全应用共享的取消令牌。内部是一个 `tokio::sync::watch` 通道，值为 `true` 表示「已取消」 |
| `struct CancelSignal` | 订阅端：既能查状态，也能 `await` 等到取消发生 |

## 逻辑要点

1. `AppState` 里放一个 `CancelToken`；谁想取消都拿它的克隆，共享同一份状态。
2. 前端点「停止」→ 调 `stop_generation` 命令 → `token.cancel()`。
3. `chat` 命令开始时 `reset()` 清掉上一次的标记，再 `subscribe()` 订阅一份。
4. 正在读网络流的地方用 `tokio::select!` 让「来数据」和「被取消」赛跑，谁先发生走哪条。
5. 被取消时循环优雅退出，**已经收到的半截文字保留**，同时推一个 `StreamEvent::Cancelled` 让前端把「生成中」收掉。

## 两个必须记住的坑

### 坑一：`send` 在没有接收端时会失败

`watch::Sender::send` 在「一个接收端都没有」时直接返回 `Err`，而且**不会更新值**（tokio 源码 `sync/watch.rs`）。

如果用 `send`，就会出现这种情况：用户点停止的那一刻，请求刚进来到还没有订阅者 → 取消信号丢了 → 停止生成**静默失效**。

所以这里用 `send_replace`：无条件写入，值一定改得掉。`cancel_sticks_even_without_subscribers` 这个测试就是专门锁住这条行为的。

### 坑二：`reset()` 必须在 `subscribe()` 之前

反过来的话，订阅者会先看到「已取消」的旧值，导致新一轮刚起步就被打断。

## 单元测试

`cd src-tauri && cargo test`，本文件 4 个测试：

| 测试 | 验证什么 |
| --- | --- |
| `cancel_is_observable_by_subscribers` | 取消后订阅者立刻能看到 |
| `cancel_sticks_even_without_subscribers` | **没人订阅时取消标记也写得进去**（防回归） |
| `reset_clears_the_flag` | reset 之后状态回到未取消 |
| `wait_blocks_until_cancelled` | 等待方能被唤醒 |

## 依赖关系

- 依赖：`tokio::sync::watch`。
- 被谁使用：[`app/state.rs`](../app/state.md)（持有令牌）、[`app/commands.rs`](../app/commands.md)（`stop_generation`）、[`agent/runner.rs`](./runner.md)（轮次检查点）、[`providers/sse.rs`](../providers/sse.md)（`select!` 打断）。

## 当前行为说明

- 全应用只有一个令牌，**同时只能有一轮生成**（前端也用 `isStreaming` 做了互斥）。
- 取消是协作式的：不会强杀线程，而是让代码在下一个检查点自己退出。
