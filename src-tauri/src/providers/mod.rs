// 声明 openai_compat 子模块，Rust 会去加载 src/providers/openai_compat.rs
pub mod openai_compat;
// 声明 sse 子模块，Rust 会去加载 src/providers/sse.rs
pub mod sse;

// 引入 async_trait 宏，它让 trait 里可以写 async fn
use async_trait::async_trait;
// 引入 serde 派生宏，让厂商枚举可以序列化、反序列化
use serde::{Deserialize, Serialize};
// 引入 Tauri 异步运行时重导出的通道发送端，本质就是 tokio 的 mpsc::Sender
use tauri::async_runtime::Sender;
// 引入统一的流式事件类型
use crate::agent::events::StreamEvent;
// 引入请求、响应数据结构
use crate::protocol::{ChatRequest, ChatResponse};
// 引入取消信号：让「用户点了停止」能传进网络读取里
use crate::agent::cancel::CancelSignal;

// 让枚举可打印调试信息、可克隆、可比较、可序列化、可反序列化
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// serde 属性：JSON 里写成小写下划线，例如 "deep_seek"
#[serde(rename_all = "snake_case")]
// 必须加 pub，因为下面 ProviderConfig 的公开字段用到了这个类型
pub enum ModelProvider {
    // 深度求索，名字必须和下面 match 分支完全一致
    DeepSeek,
    // OpenAI，是 OpenAi 而不是 OPENAI
    OpenAi,
    // 阿里通义千问
    Qwen,
}
// 为 ModelProvider 实现方法，方法名 base_url，返回该厂商的接口地址
impl ModelProvider {
    // &self 表示借用当前枚举值，&'static str 表示返回静态字符串
    pub fn base_url(&self) -> &'static str {
        // match 根据当前是哪个厂商，选择对应的地址
        match self {
            // DeepSeek 的 OpenAI 兼容接口地址
            ModelProvider::DeepSeek => "https://api.deepseek.com",
            // OpenAI 的官方接口地址
            ModelProvider::OpenAi => "https://api.openai.com/v1",
            // 阿里云百炼的 OpenAI 兼容模式地址
            ModelProvider::Qwen => "https://dashscope.aliyuncs.com/compatible-mode/v1",
        }
    }
}

// 定义「服务商配置」，创建适配器时需要用到
#[derive(Debug, Clone)]
// 结构体开始
pub struct ProviderConfig {
    // 选哪个厂商（决定 base_url）
    pub provider: ModelProvider,
    // 该厂商的 API Key，注意不要写死在代码里，建议从环境变量读
    pub api_key: String,
}

// 统一的模型接口
#[async_trait]
pub trait ChatModel: Send + Sync {
    // 唯一的流式聊天方法
    async fn chat_stream(
        // 借用适配器自身
        &self,
        // 借用请求数据
        req: &ChatRequest,
        // 通道元素从 String 换成 StreamEvent，才能把订单推给上层
        tx: Sender<StreamEvent>,
        // 取消信号：用户点「停止生成」时，靠它打断正在进行的网络读取
        cancel: CancelSignal,
        // 返回完整结果或错误
    ) -> anyhow::Result<ChatResponse>;
}

// 工厂函数：根据配置创建对应的模型适配器
pub fn create_provider(config: &ProviderConfig) -> Box<dyn ChatModel> {
    // 目前三家都兼容 OpenAI 协议，统一返回同一个适配器
    Box::new(openai_compat::OpenAiCompatProvider::new(config))
}
