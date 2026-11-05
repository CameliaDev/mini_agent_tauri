// 引入 async_trait 宏，让 impl 块里可以写 async fn
use async_trait::async_trait;
// 引入 Tauri 异步运行时重导出的通道发送端，与 trait 使用同一路径
use tauri::async_runtime::Sender;
// 从父模块引入 SSE 解析器、统一接口和服务商配置
use super::{ChatModel, ProviderConfig, sse};
// 引入统一事件类型
use crate::agent::events::StreamEvent;
// 引入请求、响应数据结构
use crate::protocol::{ChatRequest, ChatResponse};
// 引入取消信号
use crate::agent::cancel::CancelSignal;

// 定义 OpenAI 兼容协议的适配器结构体
pub struct OpenAiCompatProvider {
    // 复用的 HTTP 客户端，内部带连接池，避免每次请求都重建
    client: reqwest::Client,
    // 厂商接口根地址，由 ModelProvider::base_url() 给出
    base_url: &'static str,
    // 该厂商的 API Key，用于 Bearer 鉴权
    api_key: String,
}

// 为适配器实现构造方法
impl OpenAiCompatProvider {
    // new 接收一份配置，返回一个可用的适配器实例
    pub fn new(config: &ProviderConfig) -> Self {
        // 构造并返回 Self
        Self {
            // 新建一个 reqwest 客户端
            client: reqwest::Client::new(),
            // 按厂商取对应的接口地址
            base_url: config.provider.base_url(),
            // 克隆 API Key，因为配置是借用的，不能把字段移走
            api_key: config.api_key.clone(),
        }
    }
}

// 用 async_trait 标注，使异步方法签名和 trait 保持一致
#[async_trait]
// 为 OpenAiCompatProvider 实现统一接口 ChatModel
impl ChatModel for OpenAiCompatProvider {
    // 实现唯一的流式聊天方法
    async fn chat_stream(
        // 借用适配器自身
        &self,
        // 借用请求数据（消息历史、模型名、温度、工具清单）
        req: &ChatRequest,
        // 取得通道发送端的所有权，方法内部可反复 send
        tx: Sender<StreamEvent>,
        // 取消信号：透传给 SSE 解析器，让「停止生成」能真的打断网络读取
        cancel: CancelSignal,
        // 成功返回完整回复，失败返回 anyhow 错误
    ) -> anyhow::Result<ChatResponse> {
        // 构造要发给厂商的请求体 JSON
        let mut body = serde_json::json!({
            // 指定本次调用的模型名，例如 "deepseek-chat"
            "model": &req.model,
            // 传入完整对话历史，模型靠它记住上下文
            "messages": &req.messages,
            // 打开流式输出，厂商才会用 SSE 逐段返回
            "stream": true,
            // 温度参数，前端没传就用默认值 0.7
            "temperature": req.temperature.unwrap_or(0.7),
        });
        // 只有真的传了工具才带 tools 字段，避免某些模型对空数组报错
        if !req.tools.is_empty() {
            // 插入 tools 键，serde 会按 ToolDefinition 的结构序列化
            let _ = body
                // as_object_mut 拿到 JSON 对象的可变引用，才能插入新键
                .as_object_mut()
                // json! 生成的必然是对象，这里不会失败
                .unwrap()
                // 把工具清单（菜单）写进请求体
                .insert("tools".to_string(), serde_json::json!(&req.tools));
        }
        // 空密钥不必打到网络上。前端会把这句话换成可读的中文提示
        if self.api_key.trim().is_empty() {
            anyhow::bail!("缺少 API Key：请在项目根目录的 .env 里设置 DEEPSEEK_API_KEY 后重启");
        }
        // 发起 POST 请求并等待响应头回来
        let resp = self
            // 从复用的 client 创建一个 POST 请求
            .client
            // 拼出完整的 chat/completions 地址
            .post(format!("{}/chat/completions", self.base_url))
            // 加上 Bearer 鉴权头
            .bearer_auth(&self.api_key)
            // 把上面的 JSON 设为请求体
            .json(&body)
            // 真正发送
            .send()
            // 异步等待网络响应
            .await?;
        // 把 4xx/5xx 状态码转成错误，避免把错误页当成流解析
        let resp = resp.error_for_status()?;
        // 把响应体交给独立的 SSE 解析器，解析逻辑与原来保持一致
        // 同时把取消信号传下去：用户在生成中途点「停止」就靠它打断
        sse::parse_response(resp, &tx, cancel).await
    }
}
