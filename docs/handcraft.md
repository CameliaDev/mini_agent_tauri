# 从零手写 mini_agent（新手复刻指南）

这份指南带你用自己的手重建这个项目的每一步。每一步都写清四件事：**做什么、为什么这么做、怎么验证、失败了怎么办**。顺序就是依赖顺序，按顺序走，不要跳步。

前置条件：Node.js 20+、Rust stable；Windows 还需要 MSVC 生成工具和 WebView2 Runtime。

## 第 0 步：创建外壳

```bash
npm create tauri-app@latest
```

交互里选：项目名 `mini_agent`、前端 React + TypeScript、包管理器 npm。

生成后你只需要认识三个文件：

| 文件 | 管什么 |
| --- | --- |
| `package.json` | 前端依赖与脚本（`dev` / `build` / `tauri`） |
| `src-tauri/Cargo.toml` | 后端依赖 |
| `src-tauri/tauri.conf.json` | 端口、窗口、构建命令 |

有两个配置要记住，它们决定了后面所有调试方式：

- `tauri.conf.json` 里 `build.devUrl` = `http://localhost:1420`，`build.beforeDevCommand` = `npm run dev`；
- `vite.config.ts` 里 `server.port: 1420` 且 `strictPort: true`（端口被占用时直接失败，不会自动换）。

**验证**：`npm install`，然后 `npm run tauri dev`，应该弹出一个空窗口。

## 第 1 步：先定「数据长什么样」

新建 `src-tauri/src/protocol.rs`，把前后端要交换的数据结构写死在这一层：

```rust
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

pub struct ChatRequest {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub temperature: Option<f32>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
}
```

**为什么先写它**：后面每一层都要用这套结构。先把契约定死，后面来回改的次数最少。新手最常见的坑是一边写网络层一边改字段名，改到最后两边对不上。

**验证**：`cd src-tauri && cargo check`，能过就是语法没问题。

## 第 2 步：让「一句话」能往返一次

新建 `src-tauri/src/providers/`，先写一个统一的 trait 和一个 OpenAI 兼容实现：

```rust
#[async_trait]
pub trait ChatModel: Send + Sync {
    async fn chat_stream(
        &self,
        req: &ChatRequest,
        tx: Sender<StreamEvent>,
    ) -> anyhow::Result<ChatResponse>;
}
```

实现里只做四件事：拼请求体 → 加 Bearer 鉴权 → POST `/chat/completions` → 把响应交给解析器。

**为什么用 trait**：以后要加 OpenAI、Qwen、本地模型时，只要多写一个实现，上层一行都不用改。

**验证**：先不管流式，让它返回一段固定字符串，在 `main` 里打印出来，确认请求能通。这一步最容易卡在两个地方：API Key 没错、模型名没错（DeepSeek 是 `deepseek-chat`）。

## 第 3 步：把 SSE 解析拆成「纯函数 + 管道」

这是整个项目最容易看不懂的一块，拆法很关键：**难懂的部分写成纯函数，好测；联网的部分只做搬运。**

```rust
// 纯函数：一行 SSE 文本 -> 结构化数据
fn parse_line(line: &str) -> Option<ParsedLine> { /* ... */ }

// 纯函数：把一个工具调用分片合并进累积列表
fn merge_tool_call(pending: &mut Vec<ToolCall>, delta: &ToolCallDelta) { /* ... */ }

// 管道：读字节流、切行、调用纯函数、推事件
pub async fn parse_response(resp: Response, tx: &Sender<StreamEvent>) -> anyhow::Result<ChatResponse> { /* ... */ }
```

纯函数可以脱离网络直接测，这就是它的价值：

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn merges_split_tool_call_arguments() {
        // 第一帧给 id、工具名和参数前半段，第二帧只给后半段
        // 合并后应该等于一个完整的 JSON
    }
}
```

**验证**：`cd src-tauri && cargo test`，看到 `5 passed` 就算这一步成了。

**新手最容易踩的坑**：工具参数是**分片**送来的，必须追加不能覆盖；`id` 和工具名只在第一帧出现，来了就覆盖。

## 第 4 步：给模型一个工具

新建 `src-tauri/src/tools/mod.rs`，定义一个 trait 和一个注册表：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<String>;
}
```

注册表负责三件事：收集说明书（发给模型）、按名字找到工具、把 JSON 参数解析后执行。

每个工具单独放一个文件（`tools/builtin/time.rs`）。接进注册表时只动两处很小的登记：`builtin/mod.rs` 声明模块，`ToolRegistry` 里按 `name()` 插入。工具自己的逻辑不要写进注册表。

**验证**：写一个测试或临时打印 `registry.definitions()`，确认 JSON 里有 `type: "function"`、`name`、`parameters`。

### 当前仓库里的文件工具

上面是「先做一个工具」的最小步骤。仓库现在另外还有四个真正碰文件的工具，和 `get_current_time`、`echo` 登记在同一张表里：

| 工具 | 作用 |
| --- | --- |
| `read` | 按行读文本。`offset` 从 1 开始，`limit` 默认 400 行；超出字节上限会截断，并告诉模型下一次从哪一行继续 |
| `write` | 覆盖写入。先写同目录临时文件，再 `rename` 替换，避免写到一半留下半截文件。父目录必须已经存在 |
| `edit` | 把唯一的一段 `old_string` 换成 `new_string`。找不到、或找到多处，都报错并且不改文件 |
| `list_dir` | 只列一层。跳过名为 `node_modules`、`target`、`.git` 的目录；路径中间经过它们也会拒绝 |

路径一律先过 `src-tauri/src/tools/sandbox.rs` 的 `resolve_inside`：相对路径拼到工作区根上，`..` 和绝对路径归一化之后如果还在根外面，就直接拒绝。指向外部的符号链接同样拒绝。根目录来自环境变量 `MINI_AGENT_WORKSPACE`，不设就用进程启动时的当前目录，记在 `AppState.workspace_root` 里，由 `run_agent` 传给 `ToolRegistry::with_workspace_and_cancel`。同一份取消信号也从这里传进去。

沙箱挡的是「逃出工作区」，不是「隐藏工作区里的密钥」。`.env` 如果放在工作区根下，模型可以用 `read` 读到它。

同一张注册表里还有 `bash`。它没有 `cwd` 参数，当前目录固定是工作区根。Unix 上实际跑的是 `sh -c`，Windows 上是 `cmd /C`。默认 30 秒超时，最长 120 秒；stdout 和 stderr 各只保留前 16 KiB。`rm -rf /`、格式化磁盘、关机会在启动前被拒绝。用户点「停止」时，这条命令的子进程会被杀掉（Unix 杀进程组，Windows 用 `taskkill /T`）。这是按空白分词的黑名单，不是沙箱。

## 第 5 步：把「模型 → 工具 → 模型」接成闭环

新建 `src-tauri/src/agent/runner.rs`，这是 agent 的心脏：

```rust
for _ in 0..10 {
    let result = provider.chat_stream(&req, tx.clone()).await?;
    if result.tool_calls.is_empty() {
        let _ = tx.send(StreamEvent::Done).await;
        return Ok(result);
    }
    append_assistant(&mut req.messages, &result);
    for call in &result.tool_calls {
        let output = registry.execute(call).await
            .unwrap_or_else(|e| format!("工具执行失败: {}", e));
        append_tool_result(&mut req.messages, call, output);
    }
}
```

三个必须记住的规则：

1. 模型的回复要**放回历史**，否则下一轮它不知道自己点过什么工具；
2. 工具结果要作为一条 `role = "tool"` 的消息放回历史，并且带 `tool_call_id`；
3. 工具失败不要中断循环，把错误当成一条结果回给模型，让它自己换策略。

**验证**：临时在 `append_tool_result` 后面打印 `req.messages`，能看到历史在长大；模型点了工具时能看到工具被执行。

## 第 6 步：暴露给前端

新建 `src-tauri/src/app/`，两个文件：

- `state.rs`：读 `.env` 里的 `DEEPSEEK_API_KEY`，装进 `AppState`，用 `.manage()` 注册；
- `commands.rs`：写 `chat` 命令。

`chat` 命令的动作顺序就是全局的收尾顺序：

```rust
let (tx, mut rx) = tauri::async_runtime::channel::<StreamEvent>(64);
let forward = tokio::spawn(async move {
    while let Some(event) = rx.recv().await { let _ = on_event.send(event); }
});
let result = run_agent(state.config.clone(), tx.clone(), req).await;
drop(tx);            // 所有发送端都释放，rx 才会结束
let _ = forward.await; // 等最后几个事件推完
result.map_err(|e| e.to_string())
```

最后在 `src-tauri/src/lib.rs` 里注册命令：

```rust
tauri::Builder::default()
    .manage(AppState::from_env())
    .invoke_handler(tauri::generate_handler![greet, chat])
```

**验证**：`cargo check` 通过后，前端 `invoke("chat", ...)` 就不该再报「找不到命令」。

## 第 7 步：前端接线

按这个顺序写，每一步都能单独验证：

| 顺序 | 文件 | 做什么 |
| --- | --- | --- |
| 1 | `src/types.ts` | 把后端的结构体抄成 TS 类型；`StreamEvent` 写成联合类型 |
| 2 | `src/store.ts` | zustand 建一个 store：消息列表、`isStreaming`、`error` |
| 3 | `src/ipc/chat.ts` | `invoke` + `new Channel(...)`，把事件翻译成 store 动作 |
| 4 | `src/components/MessageList.tsx` | 渲染消息列表 |
| 5 | `src/components/Composer.tsx` | 输入框 + 发送 |
| 6 | `src/App.tsx` | 把上面几块摆在一起 |

两个必须记住的契约：

- Tauri 2 的命令参数默认 **camelCase**：Rust 的 `on_event`，前端要写 `onEvent`；
- `state` 是框架注入的，前端**不用传**。

**验证**：`npx tsc --noEmit` 通过 → `npm run tauri dev` → 窗口里能逐字看到模型回复。

## 第 8 步：修掉中文乱码（字节缓冲）

第 3 步的缓冲区如果用 `String`，遇到汉字被网络分包切开就会变成 `�`。修法只有一步：**把缓冲区换成字节**。

```rust
struct ByteBuffer { buf: Vec<u8> }

impl ByteBuffer {
    fn push(&mut self, chunk: &[u8]) { self.buf.extend_from_slice(chunk); }

    // 取出下一个完整的行；没有完整行返回 None
    fn take_line(&mut self) -> Option<String> {
        let pos = self.buf.iter().position(|b| *b == b'\n')?;
        let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
        line.pop();                                       // 去掉 \n
        if line.last() == Some(&b'\r') { line.pop(); }    // 去掉 Windows 的 \r
        Some(String::from_utf8_lossy(&line).into_owned())
    }
}
```

**关键**：只有「一整行字节」才解码。中文的 3 个字节要么都在这一行里，要么还在缓冲区里等下一块。

**验证**：写一个测试，把「你好」的 6 个字节从第 3 个字节切开喂进去，断言取出来的行仍然是「你好」且不含 `\u{FFFD}`。

---

## 第 9 步：会话持久化（SQLite）

### 9.1 先想清楚「谁存什么」

| 数据 | 存哪 |
| --- | --- |
| 会话元信息（id / 标题 / 时间） | `sessions` 表 |
| 消息（id / 角色 / 正文 / 工具调用 / 顺序） | `messages` 表 |

两张表的关系：`messages.session_id` 是外键，指向 `sessions.id`，并且带 `ON DELETE CASCADE`——删会话时消息自动跟着删。

### 9.2 建表（`src-tauri/src/storage/schema.rs`）

```sql
PRAGMA foreign_keys = ON;   -- SQLite 默认关闭，不开 CASCADE 根本不生效

CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    title      TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id         TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    seq        INTEGER NOT NULL,
    role       TEXT NOT NULL,
    content    TEXT NOT NULL DEFAULT '',
    tool_name  TEXT,
    tool_call  TEXT,
    created_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, seq);
```

**三个必须记住的点**：

1. `PRAGMA foreign_keys = ON` 要**每次连接都开**（SQLite 默认关闭）；
2. 索引建在 `(session_id, seq)` 上，否则「取某会话的消息」每次都全表扫；
3. 建表用 `IF NOT EXISTS`，启动时不用判断表在不在。

### 9.3 打开数据库（`src-tauri/src/lib.rs`）

需要真实路径才能初始化，所以要放在 `setup` 钩子里：

```rust
.setup(|app| {
    let dir = app.path().app_data_dir()?;          // 系统应用数据目录
    let storage = tauri::async_runtime::block_on(
        Storage::open(dir.join("mini_agent.db")),
    )?;
    app.manage(storage);                            // 注册成全局状态
    Ok(())
})
```

`app.path()` 和 `app.manage()` 都来自 `tauri::Manager` trait，别忘了 `use tauri::Manager;`。

### 9.4 存储句柄为什么要 `Arc<Mutex<>>`

```rust
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}
```

- `rusqlite::Connection` 是 `!Sync` 的 → 必须用 `Mutex` 包；
- Tauri 的 `State` 是不可变的，多个命令要共享同一个连接 → 必须用 `Arc`。

### 9.5 命令层只做转发

```rust
#[tauri::command]
pub(crate) async fn list_sessions(
    storage: tauri::State<'_, Storage>,
) -> Result<Vec<SessionMeta>, String> {
    storage.list_sessions().await.map_err(|e| e.to_string())
}
```

**SQL 一律用参数绑定**（`params![]`），不做字符串拼接——这是防 SQL 注入的第一道防线。

### 9.6 前端怎么接

```ts
// 启动时读回
const metas = await invoke<SessionMeta[]>("list_sessions");
useChatStore.getState().hydrateSessions(metas);

// 每轮结束后写回（放在 finally 里，成功/失败/取消都要写）
await persistSession(sessionId);
```

**为什么用「整会话覆盖」而不是「增量追加」**：消息 id 是主键，存储层用 `INSERT OR REPLACE`，重复写是安全的。比起维护「哪些是新的」这种状态，简单得多。

**验证**：`cargo test` 里用内存库（`Connection::open_in_memory()`）测这几件事——写进去能读出来、同 id 覆盖不新增、删会话消息一起没、工具调用能原样往返。

---

## 第 10 步：停止生成（协作式取消）

### 10.1 为什么要「协作式」

不能直接杀线程（Rust 里也不安全）。做法是：给一个共享的「取消标志」，让阻塞的地方自己检查它。

### 10.2 用 `watch` 通道当标志

```rust
pub struct CancelToken { tx: watch::Sender<bool> }

impl CancelToken {
    pub fn cancel(&self) { self.tx.send_replace(true); }   // 注意：不是 send
    pub fn reset(&self)  { self.tx.send_replace(false); }
    pub fn is_cancelled(&self) -> bool { *self.tx.borrow() }
    pub fn subscribe(&self) -> CancelSignal { CancelSignal { rx: self.tx.subscribe() } }
}
```

**这里有个坑，值得单独写个测试记住**：

`watch::Sender::send` 在「一个接收端都没有」时直接返回 `Err`，而且**不会更新值**。如果你把初始接收端丢掉了（`let (tx, _rx) = watch::channel(false);`），那么在还没有人订阅的那一刻点「停止」，信号就丢了，**停止生成会静默失效**。

`send_replace` 是无条件写入，值一定改得掉。用它，并写一个测试锁住这个行为：

```rust
#[tokio::test]
async fn cancel_sticks_even_without_subscribers() {
    let token = CancelToken::new();
    token.cancel();                 // 此刻没有任何订阅者
    let signal = token.subscribe();  // 现在才订阅
    assert!(signal.is_cancelled());  // 必须仍是已取消
}
```

### 10.3 打断点在哪里

真正阻塞的地方是「等网络数据」。所以用 `tokio::select!` 让两件事赛跑：

```rust
let chunk = tokio::select! {
    _ = cancel.wait_cancelled() => { cancelled = true; break 'outer; }  // 用户点了停止
    next = stream.next() => match next {
        Some(Ok(c)) => c,
        Some(Err(e)) => return Err(e.into()),
        None => break 'outer,
    },
};
```

被取消时：**保留已经收到的半截文字**，推一个 `Cancelled` 事件给前端，前端把这条回复标成「已停止」。

### 10.4 顺序：先 `reset` 再 `subscribe`

```rust
state.cancel.reset();                   // 先清掉上一轮的标记
let cancel = state.cancel.subscribe();  // 再订阅
```

反过来的话，订阅者会先看到「已取消」的旧值，这一轮刚起步就被打断。

### 10.5 主循环也要有检查点

工具执行可能耗时好几秒，那期间用户也可能改主意。所以每轮开始前、每个工具执行前都查一次 `cancel.is_cancelled()`。

**验证**：跑起来，点「停止」，应该看到：文字停在半截、「生成中」消失、请求确实结束了（后端不再继续读流）。

---

## 第 11 步：重新生成

「重新生成」= 删掉最后一条助手回复 + 用同样的历史再问一次。

```ts
// 1) 找出最后一条助手回复，和它前面那条用户消息
// 2) 内存里截断
store.truncateAfter(sessionId, lastUser.id);
// 3) 数据库里也截断
await truncateAfter(sessionId, lastUser.id);
// 4) 重发
await runAgent(history, sessionId);
```

**注意第 4 步不能走 `sendMessage`**：那条用户消息还在（截断只删了它后面的），`sendMessage` 会再 `appendUser` 一次，多出一条重复的提问。所以直接调 `runAgent`。

---

## 第 12 步：流式滚动锚定

问题：流式输出时每个字都会追加到末尾，无脑 `scrollTop = scrollHeight` 会把用户从上方「拽」回底部。

做法（ChatGPT、shadcn-chatbot-kit 都是这个思路）：

1. 监听容器的 `scroll`，算出「离底部还有多少像素」；
2. 小于阈值（80px）→ `isPinned = true`（用户想看底部）；
3. 大于阈值 → `isPinned = false`（用户往上翻了）；
4. 内容变化时，**只有 `isPinned` 为 true 才自动滚到底**；
5. 不贴底时右下角显示「回到底部」按钮。

两个实现细节：

- **用 `ref` 存一份 `isPinned`**：scroll 回调里要读最新值，但把 `isPinned` 放进 `useEffect` 依赖会让回调每次重建；
- **流式时用 `el.scrollTop = el.scrollHeight` 而不是 `scrollTo({behavior:"smooth"})`**：每个字都会触发一次，smooth 会一直排队动画、看起来卡顿。

---

## 第 13 步：打磨

到这一步功能已经通了，再做这几件事：

1. **错误可见**：把 `invoke` 的 reject 变成界面上的错误条，别只在控制台里；
2. **状态统一**：输入框内容也放进 store，空态的示例按钮才能往输入框里塞字；
3. **测试补上**：给纯函数写单测（`cargo test`），这是唯一能自动防回归的地方；
4. **写文档**：每个文件配一个同名 `.md`，写清职责、逻辑要点、依赖关系。

## 常见报错速查

| 现象 | 原因 | 怎么办 |
| --- | --- | --- |
| `Port 1420 is already in use` | 已经有 `tauri dev` 在跑 | 关掉旧窗口；或 `netstat -ano \| grep 1420` 找到 PID 后 `taskkill /PID <pid> /F` |
| `Blocking waiting for file lock on package cache` | 第二个 Cargo 在等第一个的锁 | 等第一个编译完，或结束重复启动的实例 |
| 界面空白、控制台 401 | `.env` 里没有 key，或改完 `.env` 没重启 | 确认 `DEEPSEEK_API_KEY` 存在，重启 `tauri dev` |
| `invoke` 报找不到命令 | 命令没注册 | `lib.rs` 的 `generate_handler!` 里加上它 |
| 前端传 `on_event` 没反应 | 参数名不对 | 前端改成 `onEvent`（camelCase） |
| 中文变成 `�` | 一个汉字被网络分包切开 | 用字节缓冲按 `\n` 切行，再整行 `from_utf8` |
| 模型点了工具但界面没反应 | 前端没处理 `ToolCalls` 事件 | 在 `ipc/chat.ts` 里补上这个分支 |
| 文件工具报「路径逃出工作区」 | `path` 带了 `..`、绝对路径跑到根外面，或符号链接指向外部 | 改用工作区里的相对路径；根目录用 `MINI_AGENT_WORKSPACE` |
| 点「停止」没反应 | `watch::Sender::send` 在没有接收端时不更新值 | 改用 `send_replace`，并写个测试锁住 |
| 刷新后对话没了 | 没做持久化 | 检查 `setup` 里有没有 `manage(storage)`、前端有没有调 `persistSession` |
| 界面横向溢出 | flex 子项默认 `min-width: auto` | 给滚动容器加 `min-width: 0` |
| 重新生成后多一条提问 | 走了 `sendMessage`（它会 `appendUser`） | 直接调 `runAgent`，别经过 `sendMessage` |

## 复刻完成检查清单

- [ ] `cd src-tauri && cargo test` 有测试通过（当前仓库是 49 个：SSE、取消、存储、沙箱与文件工具、bash）
- [ ] `.github/workflows/ci.yml` 会在 PR 上跑 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`，以及 `npm ci`、`npx tsc --noEmit`、`npm run build`（不打包 Tauri 安装包）
- [ ] 刷新页面后对话还在
- [ ] 点「停止」能打断生成，且已收到的半截文字保留
- [ ] 点「重新生成」不会出现重复的用户消息
- [ ] `cargo clippy --all-targets -- -D warnings` 无警告
- [ ] `cargo fmt --check` 通过
- [ ] `npx tsc --noEmit` 通过
- [ ] 窗口里能逐字看到模型回复
- [ ] 模型点工具时能看到工具卡片，工具结果能被它继续引用
- [ ] 每个源文件旁边有一份同名 `.md` 说明
