# mini_agent

一个跑在 Tauri 桌面窗口里的本地 Agent：输入一句话 → 模型流式回复 → 模型点名调用工具 → 工具结果回灌模型 → 继续回答。

前端 React 19 + TypeScript + Vite，后端 Rust + Tauri 2，模型走 DeepSeek 的 OpenAI 兼容接口，会话落在本地 SQLite。

---

## 0. 项目治理：AI 全权开发

> **本项目完全由 AI 发挥，AI 拥有全部工程决策与改动权限。**

这句话不是口号，是实际的开发方式，也是阅读本项目的前提：

| 角色 | 负责什么 |
| --- | --- |
| 用户 | 提出目标、给方向、验收结果、决定下一步做什么 |
| AI | 技术选型、架构分层、接口契约、全部源码、单元测试、文档体系、构建验证、Git 版本管理 |

具体体现：

- **架构是 AI 定的**：为什么拆 `providers` / `agent` / `tools` / `storage` / `app` 五层、为什么用 trait 抽象模型、为什么把 IPC 收敛到一个目录，理由都写在 [架构决策记录](#22-架构决策记录adr)里，不靠口头传承。
- **代码是 AI 写的**：包括修复过程中自己发现的隐患（`watch::Sender::send` 在无接收端时不更新值、`isStreaming` 从未置真、CSS 覆盖顺序导致遮罩不显示等）。
- **文档是 AI 维护的**：每个源文件旁边有一份同名 `.md`，采用统一结构，代码改了文档同步改。
- **质量由 AI 验证**：`cargo test` / `cargo clippy` / `tsc --noEmit` / `npm run build` / 文档断链检查，全部跑过才交付。
- **版本管理由 AI 执行**：`.gitignore` 的加固、首次提交都由 AI 完成，密钥不入库是硬约束。

**项目定位**：这既是一个能跑的桌面 Agent，也是一份**给使用者后期自己动手复刻的参考实现**。因此代码刻意写得偏「教科书」：注释解释「为什么」而不只是「是什么」，难懂的逻辑拆成可单测的纯函数，每个文件都有说明书。若目标是极致精简，很多注释和分层可以合并——那是另一个取舍，不是本项目忘了做。

---

## 1. 快速开始

### 前置

| 平台 | 需要 |
| --- | --- |
| 通用 | Node.js 20+、Rust stable（edition 2024，需要 1.85+）、npm |
| Windows | MSVC 生成工具、WebView2 Runtime |
| macOS | Xcode Command Line Tools |
| Linux | `webkit2gtk-4.1`、`libayatana-appindicator3` 等 Tauri 系统依赖 |

### 安装与运行

```bash
npm install
cp .env.example .env      # Windows 用 copy .env.example .env
```

编辑 `.env`，填入自己的密钥：

```
DEEPSEEK_API_KEY=sk-你的密钥
```

```bash
npm run tauri dev
```

窗口打开后直接输入。改前端代码热更新；改 Rust 代码会重新编译并重启窗口。

> **两个高频坑**
> - 改完 `.env` 必须**重启** `tauri dev`：`dotenvy` 只在进程启动时读一次，热更新不会重新加载。
> - 报 `Port 1420 is already in use`：说明已有一个 `tauri dev` 在跑，关掉旧窗口或结束旧进程，**不要改端口**（`vite.config.ts` 与 `tauri.conf.json` 是一对的）。

### 常用命令

| 命令 | 作用 |
| --- | --- |
| `npm run dev` | 只跑前端（浏览器里看，没有后端能力） |
| `npm run build` | `tsc && vite build`，类型检查通过后才打包，产物在 `dist/` |
| `npm run tauri dev` | 日常开发 |
| `npm run tauri build` | 打包安装包 |
| `cd src-tauri && cargo check` | 后端类型检查 |
| `cd src-tauri && cargo test` | 后端单元测试（41 个） |
| `cd src-tauri && cargo fmt --check` | 后端格式，与 CI 相同 |
| `cd src-tauri && cargo clippy --all-targets -- -D warnings` | 后端 lint，与 CI 相同 |
| `npx tsc --noEmit` | 前端类型检查 |

---

## 2. 企业级设计

### 2.1 分层与依赖方向

后端按「**协议 → 适配 → 编排 → 持久化 → 宿主**」单向依赖，每一层只认识下一层的抽象，不反向引用：

```
        ┌─────────────────────────────────────────────┐
        │  app/          Tauri 宿主层（命令、状态）      │
        │  commands.rs / state.rs / lib.rs              │
        └───────────────┬─────────────────────────────┘
                        │ 调用
        ┌───────────────▼─────────────────────────────┐
        │  agent/        编排层（主循环、事件、取消）    │
        │  runner.rs / events.rs / cancel.rs            │
        └──────┬──────────────────┬───────────────────┘
               │                  │
   ┌───────────▼──────┐  ┌───────▼──────────┐
   │  providers/      │  │  tools/          │
   │  模型适配 + SSE  │  │  工具 trait/注册 │
   └───────────┬──────┘  └───────┬──────────┘
               └────────┬─────────┘
                        │ 共用
        ┌───────────────▼─────────────────────────────┐
        │  protocol.rs   契约层（与厂商无关的数据结构） │
        └─────────────────────────────────────────────┘

        ┌─────────────────────────────────────────────┐
        │  storage/      持久化层（SQLite，独立可测）  │
        └─────────────────────────────────────────────┘
```

前端同样单向：

```
  components/ ──► store.ts ◄── ipc/ ──► @tauri-apps/api
       │             ▲
       └── hooks/ ───┘
```

**硬规则**：除了 `src/ipc/`，任何前端组件都不许 `import "@tauri-apps/api"`。后端协议变了只改 `ipc/` 两个文件，组件零改动。

| 分层 | 关键约束 |
| --- | --- |
| 契约层 `protocol.rs` | 前后端唯一的数据字典；改字段名要同步 `src/types.ts` |
| 适配层 `providers/` | 通过 `ChatModel` trait 抽象，换厂商只加一个实现，编排层不动 |
| 编排层 `agent/` | 不认识 HTTP、不认识 SQLite，只管「问模型 → 跑工具 → 再问」 |
| 持久化层 `storage/` | SQL 全部参数绑定；方法都短、都用内存库单测 |
| 宿主层 `app/` | 只做「取状态 → 调下层 → 把错误转成字符串」，不含业务逻辑 |
| 前端 `ipc/` | 唯一 IPC 边界；流式事件在这里翻译成 store 动作 |

### 2.2 架构决策记录（ADR）

| # | 决策 | 为什么 | 代价 / 边界 |
| --- | --- | --- | --- |
| 1 | 契约先行：先写 `protocol.rs` 再写网络层 | 前后端字段最容易在联调时反复改，先把契约定死成本最低 | 改字段要同步两处 |
| 2 | 模型用 `ChatModel` trait 抽象 | 以后接 OpenAI / Qwen / 本地模型只需新增实现 | 多一层间接，初学者要多读一个文件 |
| 3 | SSE 解析拆成「纯函数 + 管道」 | 纯函数脱离网络可单测，是本项目的回归防线 | `parse_response` 仍无法单测 |
| 4 | 字节缓冲而非字符串缓冲 | 汉字 3 字节被 TCP 切开会变 `U+FFFD`；攒够整行再解码 | 缓冲区无上限（SSE 行实际很短） |
| 5 | 协作式取消（watch 令牌 + `select!`） | 不能强杀线程；让阻塞点在下一个检查点自己退出，并保留半截文字 | 全应用单令牌，同时只能一轮生成 |
| 6 | `send_replace` 而非 `send` | `watch::Sender::send` 在无接收端时返回 Err **且不更新值**，会让「停止」静默失效 | 无；已有回归测试锁住 |
| 7 | 前端状态单一来源（zustand） | 组件之间不直接通信，空态示例按钮才能往输入框塞字 | 所有状态集中在一处，文件偏大 |
| 8 | 流式动作按「消息 id」反查会话 | 用户中途切会话时，文字片段仍写到原来的那条消息上 | 每个事件都要带 `anchorId` |
| 9 | 会话落 SQLite（后端）而非 `localStorage` | 事务性写入、结构化查询、数据随应用数据目录走 | 每次落库逐条 IPC，消息多时偏慢 |
| 10 | `INSERT OR REPLACE` + 消息 id 主键 | 重试/重落库不会产生重复行，前端不用维护「哪些是新的」 | 无批量事务 |
| 11 | 滚动锚定 `useStickToBottom` | 用户上翻时不能被硬拽回底部；不贴底时给「回到底部」 | 阈值 80px 是经验值 |
| 12 | 助手消息无气泡 + 工具用 Marker 行 | 对标成熟桌面 AI 客户端的长文可读性 | 与「所有消息都是气泡」的直觉不同 |
| 13 | 深色模式只覆盖 CSS 变量 | 新增组件不用考虑配色分支 | 个别硬编码颜色需单独覆盖 |

### 2.3 质量门禁

| 检查 | 命令 | 当前状态 |
| --- | --- | --- |
| 后端单元测试 | `cd src-tauri && cargo test` | **41 passed**（SSE 6 / 取消 4 / 存储 7 / 沙箱与文件工具 24） |
| 后端格式 | `cd src-tauri && cargo fmt --check` | 通过 |
| 后端 lint | `cargo clippy --all-targets -- -D warnings` | 无警告 |
| 前端类型 | `npx tsc --noEmit` | 通过 |
| 前端构建 | `npm run build` | 通过 |
| CI | [`.github/workflows/ci.yml`](./.github/workflows/ci.md) | PR 与 `main` 上跑上面这几项，不打包 Tauri 安装包 |

测试刻意集中在四类「最容易悄悄坏掉」的地方：SSE 分片解析、取消信号语义、SQLite 读写与级联删除、工作区沙箱（路径逃逸、截断、唯一匹配）。`cargo test` 不需要任何外部服务或 API Key。文件工具的测试用临时目录，不碰仓库里的文件。

### 2.4 安全与密钥

| 项 | 做法 |
| --- | --- |
| API Key | 只放 `.env`；`.gitignore` 排除 `.env` / `.env.*`，仅放行不含真实值的 `.env.example` |
| SQL 注入 | 所有 SQL 用 `params![]` 绑定，不拼接字符串 |
| XSS | `react-markdown` 默认转义 HTML，模型输出里的 `<script>` 只显示为文本 |
| IPC 权限 | `capabilities/default.json` 只给 `core:default` + `opener:default`。窗口没有 Tauri 文件系统 / shell 权限 |
| 文件工具沙箱 | `read` / `write` / `edit` / `list_dir` 走 Rust `std::fs`，只允许访问 `AppState.workspace_root`。`..`、根外绝对路径、指向外部的符号链接都会拒绝 |
| 工作区内的密钥 | 沙箱不隐藏根目录里面的文件。`.env` 如果放在工作区里，模型可以用 `read` 读到。把 `MINI_AGENT_WORKSPACE` 指到不含密钥的目录，或不要把密钥放进工作区 |
| 密钥读取 | 后端 `app/state.rs` 通过 `dotenvy` 注入 `AppState`，前端永远拿不到 Key |
| 数据库位置 | 系统应用数据目录（`%APPDATA%/<应用名>/mini_agent.db`），不在仓库里 |

**待改进**：`tauri.conf.json` 的 `security.csp` 目前为 `null`，对标严格企业级应配置 CSP；`identifier` 仍是模板默认值。见[已知限制](#7-已知限制与演进路线)。

### 2.5 错误处理与可观察性

- 后端统一用 `anyhow::Result`，命令边界 `map_err(|e| e.to_string())` 转成前端可读字符串。
- 前端 `invoke` 的 reject 变成顶部错误条，不只在控制台里报错。
- 工具执行失败**不中断循环**：把错误文本当成工具结果回给模型，让它自己换策略。
- 流式通道关闭、SSE 坏帧、401、轮数超限都有明确去向；空助手气泡在 `finish` / `fail` 里被清掉。

### 2.6 内置工具

| 工具 | 作用 |
| --- | --- |
| `get_current_time` | 当前本地时间，无参数 |
| `echo` | 把传入文本原样返回，用来验证带参数的工具调用 |
| `read` | 按行读工作区里的 UTF-8 文本。`offset` 从 1 开始，`limit` 默认 400 行，超限会截断并告诉模型下一行 |
| `write` | 覆盖写入。先写同目录临时文件，再 `rename` 替换。父目录必须已经存在 |
| `edit` | 把唯一的一段 `old_string` 换成 `new_string`。找不到或找到多处都报错，并且不改文件 |
| `list_dir` | 只列一层。跳过名为 `node_modules`、`target`、`.git` 的目录，路径中间经过它们也会拒绝 |

工作区根来自 `MINI_AGENT_WORKSPACE`，不设则是进程启动时的当前目录，记在 `AppState.workspace_root` 里。细节在 [`src-tauri/src/tools/sandbox.md`](./src-tauri/src/tools/sandbox.md)。

---

## 3. 界面

桌面 AI 客户端常见的两栏结构：**左侧会话栏（今天 / 昨天 / 更早分组）+ 右侧对话区**。

- 布局参考：[Chatbox](https://github.com/chatboxai/chatbox)、[NextChat](https://github.com/ChatGPTNextWeb/NextChat)、[LobeChat](https://github.com/lobehub/lobe-chat)
- 消息排版参考：[shadcn/ui chat 组件](https://ui.shadcn.com/docs/changelog/2026-06-chat-components)（`Message` / `Bubble` / `Marker`、滚动渐隐、shimmer 状态文字）

```
┌────────────┬──────────────────────────────────┐
│ 会话栏      │ 顶栏：模型名 / 清空对话            │
│ 新建对话    ├──────────────────────────────────┤
│ 今天        │                                  │
│  · 会话 A   │  消息区（可滚动，两端渐隐）        │
│  · 会话 B   │                                  │
│ 昨天        │                                  │
│ 更早        ├──────────────────────────────────┤
│            │ 输入区（居中卡片）                │
└────────────┴──────────────────────────────────┘
```

**响应式**：> 860 CSS px 侧栏常驻；≤ 860 px 变成抽屉，默认收起，展开时有遮罩，点遮罩或 ☰ 收起。窗口跨断点时 `matchMedia` 自动切换（`App.tsx` 的 `NARROW_QUERY` 与 `App.css` 的媒体查询必须同步改）。

> 系统缩放到 125% 时 CSS 像素会变小，物理宽度看着不小也可能跨过断点——这是 CSS 像素的正常行为，不是布局 bug。

---

## 4. 数据流

一次发送：

```
Composer → sendMessage(text)
  1. store.appendUser            当前会话加一条用户消息（首条顺带当会话标题）
  2. store.startAssistant        建空助手消息，拿到 assistantId，isStreaming 置真
  3. new Channel(callback)       必须在 invoke 之前注册，否则早期事件会丢
  4. invoke("chat", {req,onEvent})  → 后端 commands::chat
       ├─ 两级通道：agent 推内部 mpsc → 转发任务推 Tauri Channel → 前端
       ├─ agent::run_agent 循环：chat_stream → 有工具就执行并回填历史 → 再问
       └─ sse::parse_response 用 select! 同时等「来数据」和「被取消」
  5. 事件翻译：TextDelta→appendDelta / ToolCalls→addToolCards / Done|Cancelled→finish
  6. invoke resolve/reject → finish / fail 兜底
  7. finally → persistSession(sessionId) 落库（成功/失败/取消都写）
```

后端的 agent 闭环：

```
ChatRequest.messages → HTTP/SSE → ChatResponse
          ↑                            ↓
          └── assistant / tool 消息回填 ┘ → 再发一次 → Done → 窗口
```

---

## 5. 三个关键能力

| 能力 | 实现 |
| --- | --- |
| **会话持久化** | 后端 [`storage/`](./src-tauri/src/storage/mod.md)：SQLite 两张表 + 外键级联；前端 [`ipc/sessions.ts`](./src/ipc/sessions.md) 启动 hydrate、每轮落库。刷新/重启不丢 |
| **停止 / 重新生成** | 后端 [`agent/cancel.rs`](./src-tauri/src/agent/cancel.md) 取消令牌 + `tokio::select!` 打断网络读取；前端「停止」调 `stop_generation`，「重新生成」截断旧回复后走公共 `runAgent`（**不能走 `sendMessage`**，否则会重复插入用户消息） |
| **流式滚动锚定** | [`hooks/useStickToBottom.ts`](./src/hooks/useStickToBottom.md)：贴底才自动跟随，上翻时不抢滚动位置并出现「回到底部」 |

---

## 6. 目录结构与文档体系

```
mini_agent/
├── README.md                    ← 本文件（项目总览 / 架构 / 治理 / 复刻索引）
├── index.html / package.json / vite.config.ts / tsconfig*.json
├── .env.example                 ← 密钥模板（入库）；.env 不入库
├── .gitignore
├── .github/workflows/ci.yml     CI：fmt / clippy / test，以及 tsc / vite build
├── src/                         前端
│   ├── types.ts                 前后端共用的数据结构
│   ├── store.ts                 唯一状态仓库：会话 + 消息
│   ├── ipc/  chat.ts            流式聊天：invoke + Channel
│   │         sessions.ts        会话持久化
│   ├── hooks/ useStickToBottom.ts
│   ├── components/  Sidebar / MessageList / EmptyState / Composer
│   ├── App.tsx / App.css
│   └── main.tsx
├── src-tauri/                   后端
│   ├── Cargo.toml / tauri.conf.json / capabilities/ / icons/ / build.rs
│   └── src/
│       ├── protocol.rs          契约层
│       ├── providers/           openai_compat.rs + sse.rs
│       ├── tools/               trait + 注册表 + sandbox.rs + builtin/
│       ├── agent/               runner / events / cancel / session
│       ├── storage/             mod.rs + schema.rs
│       ├── app/                 commands.rs + state.rs
│       └── lib.rs / main.rs
└── docs/handcraft.md            ← 从零手写复刻指南（13 步）
```

### 文档约定（重要）

**每个源文件旁边都有一份同名 `.md`**，采用统一七段结构，方便按图索骥：

1. 源码链接与分层
2. 职责
3. 包含的定义（表格列出所有函数/类型）
4. 逻辑要点（编号、解释「为什么」）
5. 单元测试（有测试的文件）
6. 依赖关系（依赖谁 / 被谁使用）
7. 当前行为说明（限制、残留、以后怎么改）

| 类别 | 文档 | 份数 |
| --- | --- | --- |
| 后端源码 | `src-tauri/src/**/*.md` | 25 |
| 前端源码 | [`src/README.md`](./src/README.md)（总览）+ 各文件 `.md` | 15 |
| 根 / 后端配置 / 静态资源 | `index.md`、`package.md`、`tsconfig.md`、`Cargo.md`、`tauri.conf.md`、`.gitignore.md`、`.github/workflows/ci.md`、`icons/README.md` 等 | 22 |
| 复刻指南 | [`docs/handcraft.md`](./docs/handcraft.md) | 1 |
| **合计** | | **63** |

统计口径：`git ls-files '*.md'`（即实际入库的文档）。`.codebuddy/skills/` 下的 SKILL.md 属于 IDE 技能数据，不入库也不计入。

**两类例外**（企业级做法是明确写出例外，而不是机械凑数）：

- **自动生成物**（`gen/schemas/*.json`、`package-lock.json`、`Cargo.lock`）→ 锁文件写同名说明解释「勿手改」；`gen/schemas/*.json` 本身不入库，用目录级 [`gen/schemas/README.md`](./src-tauri/gen/schemas/README.md) 统一说明（该文件特意放行，保证链接有效）。
- **二进制资源**（`icons/*.png`、`*.ico`）→ 用目录级 [`icons/README.md`](./src-tauri/icons/README.md) 覆盖，逐张 PNG 写说明没有信息增量。

复刻时建议的阅读顺序：`README.md` → `src/README.md` → `protocol.md` + `types.md` → `runner.md` → `sse.md` → `commands.md` → 其余按需。

---

## 7. 复刻路线

[`docs/handcraft.md`](./docs/handcraft.md) 是给「自己从零写一遍」准备的，每一步都写清**做什么 / 为什么 / 怎么验证 / 失败了怎么办**：

| 阶段 | 步骤 | 目标 |
| --- | --- | --- |
| 打地基 | 0–2 | 建外壳、定契约、让一句话往返一次 |
| 核心 | 3–6 | SSE 纯函数拆分、加工具、接成 agent 闭环、暴露给前端 |
| 打通 | 7 | 前端接线（类型 → store → ipc → 组件） |
| 修真问题 | 8 | 字节缓冲修中文乱码 |
| 产品化 | 9–12 | SQLite 持久化、停止生成、重新生成、滚动锚定 |
| 收尾 | 13 | 错误可见、状态统一、补测试、写文档 |

另附**常见报错速查表**（端口占用、Cargo 文件锁、401、`on_event` 参数名、中文乱码、CSS 覆盖顺序等）和**完成检查清单**。

核心方法论只有一句：**先写成能跑通的最小版本，再一层层加能力，每加一层就验证一次**。组件化、抽象层、持久化都是「同一段代码写到第二遍」时才抽出来的，不是开工第一天就该建的目录。

---

## 8. 已知限制与演进路线

| 现状 | 影响 | 建议下一步 |
| --- | --- | --- |
| `security.csp: null` | 未启用内容安全策略 | 配置严格 CSP（当前不渲染用户 HTML，风险较低） |
| `identifier` 为模板默认 | 不能正式发布 | 改成自有域名反写 |
| 每次落库逐条 `append_message` | 长对话 IPC 次数多 | 改批量事务 / 只增量写新消息 |
| 不支持会话重命名 | 标题自动取首条消息前 24 字 | 加右键重命名 + `save_session` 更新 |
| 工具执行结果界面不可见 | 只显示「模型点了哪个工具」 | 新增 ToolResult 事件与展示 |
| 单令牌、同时只能一轮生成 | 不能并发多会话生成 | 改成按会话 id 持有令牌 |
| SSE 未合并同一事件的多行 `data` | 少数厂商兼容问题 | 按空行切事件再合并 `data` |
| 未裁剪历史 | 长对话 token 膨胀 | 按 token 预算裁剪或摘要 |
| 模板残留依赖与图标 | 安装体积、品牌一致性 | 清理 `package.json` 未用包、换自有图标 |
| 没有 bash 工具 | 模型不能在工作区里跑测试或 git | 留待后续，本轮只做文件工具和 CI |
| 界面打磨未做完 | 停止后的收尾、工具卡片时序、空状态引导还要再核 | 留待后续的 UX 打磨 |

---

## 9. 版本管理

- `.env`、`.env.*`、`src-tauri/target/`（可达数 GB）、`node_modules/`、`dist/`、本地 `*.db` 均不入库；规则见 [`.gitignore.md`](./.gitignore.md)。
- `.codebuddy/` 是项目相关的工作区数据，**不删除**，但作为本机 IDE 配置不纳入版本库。
- `.vscode/extensions.json` 是唯一放行的 IDE 配置（推荐 `tauri-vscode` + `rust-analyzer`），含本机绝对路径的 `settings.json` 不放行。
- 提交前自检：`git status --short` 中不应出现 `.env`、`target/`、`node_modules/`；用 `git check-ignore -v <路径>` 可查命中了哪条规则。

---

## 10. 这份项目能拿来做什么

- **当桌面 Agent 用**：配好 Key 直接跑，支持多会话、流式输出、工具调用、停止/重新生成、刷新不丢。
- **当 Tauri 2 入门样例**：看 `invoke` / `Channel` / `State` / `setup` 怎么配合，看 Rust 与 TypeScript 的契约怎么对齐。
- **当工程实践参考**：看分层、ADR、纯函数单测、协作式取消、参数绑定、响应式断点、CSS 变量体系。
- **当复刻教材**：照 `docs/handcraft.md` 从第 0 步写一遍，每步都有验证点；写不下去时对照同名 `.md` 看设计理由。

> 所有工程决策由 AI 做出并记录在此；使用者保留最终目标设定与验收权。
