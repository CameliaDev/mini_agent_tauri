// 引入服务商配置与厂商枚举
use crate::providers::{ModelProvider, ProviderConfig};
// 引入取消令牌：全应用共享一份，「停止生成」命令靠它打断正在跑的请求
use crate::agent::cancel::CancelToken;

// 定义应用的共享状态，Tauri 会把它注入到每个 command
pub struct AppState {
    // 模型服务商配置，含厂商枚举和 API Key
    pub config: ProviderConfig,
    // 取消令牌：chat 命令订阅它，stop 命令触发它
    pub cancel: CancelToken,
}

impl AppState {
    // 从环境变量装配配置，启动时调用一次
    pub fn from_env() -> Self {
        // 尝试加载项目根目录的 .env 文件，没有也不影响启动
        let _ = dotenvy::dotenv();
        // 读取 API Key，优先 DEEPSEEK_API_KEY，其次 MINI_AGENT_API_KEY
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .or_else(|_| std::env::var("MINI_AGENT_API_KEY"))
            .unwrap_or_default();
        Self {
            // 服务商配置
            config: ProviderConfig {
                // 目前固定 DeepSeek
                provider: ModelProvider::DeepSeek,
                // API Key
                api_key,
            },
            // 取消令牌：初始状态是「未取消」
            cancel: CancelToken::new(),
        }
    }
}
