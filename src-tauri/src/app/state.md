# `state.rs`

- 源码：[`state.rs`](./state.rs)
- 分层：Tauri 宿主层 / 全局状态与配置
- 拆分来源：原 `lib.rs` 里的 `AppState` 定义和 `run()` 中的 `.env` / API Key 读取

## 职责

保存应用运行期间需要共享的配置，并在启动时把它装配出来。配置里含厂商枚举与 API Key，模型适配器每次请求都会用到。

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `pub struct AppState` | Tauri 注入到每个 command 的共享状态：`config`（配置与密钥）+ `cancel`（取消令牌） |
| `impl AppState::from_env()` | 从环境变量装配配置的构造函数 |

## 逻辑要点

1. `dotenvy::dotenv()` 尝试从当前工作目录向上找 `.env` 并加载，失败也不影响启动（返回值被忽略）。
2. API Key 优先读 `DEEPSEEK_API_KEY`，读不到再读 `MINI_AGENT_API_KEY`，都没有就用 `unwrap_or_default()` 得到空字符串。
3. `provider` 固定为 `ModelProvider::DeepSeek`，`base_url()` 会解析成 `https://api.deepseek.com`。
4. [`lib.rs`](../lib.md) 用 `.manage(AppState::from_env())` 注册状态；[`commands.rs`](./commands.md) 用 `tauri::State<'_, AppState>` 取出 `config.clone()`。
5. `cancel` 是全应用共享的一份 [`CancelToken`](../agent/cancel.md)：`chat` 命令订阅它，`stop_generation` 命令触发它。

## 依赖关系

- 依赖：[`providers`](../providers/mod.md) 的 `ModelProvider`、`ProviderConfig`，[`agent/cancel.rs`](../agent/cancel.md) 的 `CancelToken`，以及 `dotenvy`。
- 被谁使用：[`lib.rs`](../lib.md)、[`app/commands.rs`](./commands.md)。

## 当前行为说明

- `ModelProvider` 还定义了 `OpenAi`、`Qwen` 两个变体，但这里写死 `DeepSeek`，所以那两个分支目前不可达。
- API Key 为空时不会在启动阶段报错，仍会带着空 Bearer 令牌发请求。
- 打包后进程的当前工作目录不一定等于项目根目录，`.env` 的查找位置由运行时 cwd 决定。
