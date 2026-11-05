// ============================================================================
// 取消信号：让「用户点了停止」这件事，能传进正在跑的网络流里
//
// 为什么需要它：原来的实现里，一旦 chat 命令跑起来就只能等它跑完。
// 用户关掉页面时，模型还在继续生成，白花钱。
//
// 做法（协作式取消，参考 tokio-util 的 CancellationToken）：
//   1. AppState 里放一个 CancelToken，所有人拿到的都是它的克隆，共享同一份状态；
//   2. 前端点「停止」→ 调用 stop_generation 命令 → token.cancel()；
//   3. 正在读网络流的地方 select! 等着两个分支：来数据 / 被取消，谁先发生算谁；
//   4. 被取消时循环优雅退出，把已经收到的半截文字保留下来。
//
// 为什么不用 tokio_util::sync::CancellationToken：
//   它需要额外依赖，而这里只需要一个 bool + 一个通知机制，自己写更好读也好改。
// ============================================================================

// 引入 tokio 的 watch 通道：既能读当前值，也能等待「值变了」的通知
use tokio::sync::watch;

// 定义取消令牌：内部是一个 watch 通道，发送端持有 true 表示「已取消」
// Default 让它可以写成 `CancelToken::default()`，等价于 new()
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    // 发送端：调用 cancel() 就是往里写 true
    tx: watch::Sender<bool>,
}

impl CancelToken {
    // 创建一个全新的取消令牌，初始状态是「未取消」
    pub fn new() -> Self {
        // 建立通道；第二个返回值是接收端，这里用不到，先丢掉
        let (tx, _rx) = watch::channel(false);
        // 包进结构体
        Self { tx }
    }

    // 发出「取消」信号：把值改成 true 并通知所有等待者
    pub fn cancel(&self) {
        // 用 send_replace 而不是 send
        // 原因：watch 的 send 在「一个接收端都没有」时会直接返回 Err，
        // 而且**不会更新值**。那样一来，若此刻还没有人订阅（请求刚进来、
        // 或者订阅者刚被 drop），取消信号就丢了，停止生成会静默失效。
        // send_replace 是无条件写入，值一定改得掉。
        self.tx.send_replace(true);
    }

    // 清掉取消标记：新一轮对话开始前调用
    pub fn reset(&self) {
        // 同样用 send_replace，保证「清标记」这一步永远生效
        self.tx.send_replace(false);
    }

    // 查询当前是否已被取消
    pub fn is_cancelled(&self) -> bool {
        // borrow 拿到当前值的引用，读出 bool 后立刻拷贝出来
        *self.tx.borrow()
    }

    // 订阅一份信号：只有订阅者才能「等待取消」
    pub fn subscribe(&self) -> CancelSignal {
        // subscribe 会返回一个接收端，它能看见后续所有变化
        CancelSignal {
            // rx 内部字段是私有的，这里在同一个模块里可以直接构造
            rx: self.tx.subscribe(),
        }
    }
}

// 定义取消信号的订阅端：既能查状态，也能 await 变化
// Clone 是必须的：每一轮模型请求都要一份独立的订阅
#[derive(Debug, Clone)]
pub struct CancelSignal {
    // 接收端：可以读当前值，也可以等它变
    rx: watch::Receiver<bool>,
}

impl CancelSignal {
    // 查询是否已取消（同步判断，用于循环里的检查点）
    pub fn is_cancelled(&self) -> bool {
        // 读当前值
        *self.rx.borrow()
    }

    // 等待「被取消」这件事发生
    // 返回 true 表示确实收到了取消；false 表示发送端已被销毁（不会再有取消）
    pub async fn wait_cancelled(&mut self) -> bool {
        // 循环：先看当前状态，再等变化
        loop {
            // 已经取消了就不用等
            if self.is_cancelled() {
                // 返回「已取消」
                return true;
            }
            // 等下一次变化；发送端被销毁时返回 Err
            if self.rx.changed().await.is_err() {
                // 没人再能发取消信号了
                return false;
            }
        }
    }
}

// ============================================================================
// 单元测试：取消信号的行为必须可靠，尤其是「没人订阅时也要写得进去」
// 运行：cd src-tauri && cargo test
// ============================================================================
#[cfg(test)]
mod tests {
    // 引入被测的类型
    use super::*;

    // 取消后，订阅者能看到「已取消」
    #[tokio::test]
    async fn cancel_is_observable_by_subscribers() {
        // 建一个令牌
        let token = CancelToken::new();
        // 订阅一份
        let mut signal = token.subscribe();
        // 初始不是取消状态
        assert!(!signal.is_cancelled());
        // 发出取消
        token.cancel();
        // 订阅者立刻能看到
        assert!(signal.is_cancelled());
        // 等待也会立刻返回 true
        assert!(signal.wait_cancelled().await);
    }

    // 关键回归测试：即使当前一个订阅者都没有，取消标记也必须写得进去
    // （watch 的 send 在没有接收端时会返回 Err 且不更新值，send_replace 不会）
    #[tokio::test]
    async fn cancel_sticks_even_without_subscribers() {
        // 建一个令牌，注意这里不订阅
        let token = CancelToken::new();
        // 发出取消
        token.cancel();
        // 再订阅：订阅者应该看到「已取消」
        let signal = token.subscribe();
        // 必须是已取消状态
        assert!(signal.is_cancelled());
    }

    // reset 之后状态回到「未取消」
    #[tokio::test]
    async fn reset_clears_the_flag() {
        // 建令牌
        let token = CancelToken::new();
        // 先取消
        token.cancel();
        // 确认已取消
        assert!(token.is_cancelled());
        // 清标记
        token.reset();
        // 确认已清掉
        assert!(!token.is_cancelled());
    }

    // 等待取消：先等，等到了才返回
    #[tokio::test]
    async fn wait_blocks_until_cancelled() {
        // 建令牌并订阅
        let token = CancelToken::new();
        let mut signal = token.subscribe();
        // 另开一个任务，稍后触发取消
        let trigger = token.clone();
        tokio::spawn(async move {
            // 让出一次调度，确保等待方已经进入等待状态
            tokio::task::yield_now().await;
            // 触发取消
            trigger.cancel();
        });
        // 等待取消信号，应该返回 true
        assert!(signal.wait_cancelled().await);
    }
}
