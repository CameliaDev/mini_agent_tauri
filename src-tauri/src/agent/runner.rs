// ============================================================================
// 这一层是 agent 的心脏：只做一件事——把「模型说话」和「工具执行」接成闭环
//
// 它不认识 Tauri、不认识 HTTP、不认识前端：
//   输入：一份服务商配置 + 一条事件通道 + 一份请求
//   输出：把过程事件往通道里推，最后返回模型的最终答复
// 正因为不依赖任何外部框架，这一层可以单独读懂、单独测试
// ============================================================================

// 引入 Tauri 异步运行时重导出的通道发送端
use tauri::async_runtime::Sender;
// 引入流式事件枚举
use crate::agent::events::StreamEvent;
// 引入会话历史追加函数
use crate::agent::session::{append_assistant, append_tool_result};
// 引入聊天请求、响应数据结构
use crate::protocol::{ChatRequest, ChatResponse};
// 引入模型适配器工厂与服务商配置
use crate::providers::{create_provider, ProviderConfig};
// 引入工具注册表
use crate::tools::ToolRegistry;
// 引入取消信号
use crate::agent::cancel::CancelSignal;

// 定义 agent 循环：模型点菜 -> 我们做菜 -> 把结果端回去 -> 再问一次
pub async fn run_agent(
    // 模型服务商配置（厂商 + API Key）
    config: ProviderConfig,
    // 内部事件通道，由 command 层负责转发到前端
    tx: Sender<StreamEvent>,
    // 聊天请求，循环中要往 messages 里追加消息所以是可变的
    mut req: ChatRequest,
    // 取消信号：用户点「停止生成」后，每轮开始前都会检查它
    cancel: CancelSignal,
) -> anyhow::Result<ChatResponse> {
    // 1) 准备两样东西：工具注册表（菜单）和模型适配器（跑腿的）
    // 创建内置工具注册表
    let registry = ToolRegistry::builtin();
    // 按配置创建对应的模型适配器
    let provider = create_provider(&config);
    // 把工具清单（菜单）注入请求，模型才知道有哪些工具可用
    req.tools = registry.definitions();

    // 2) 循环本体：一轮 = 一次请求 + 可能的工具执行
    // 最多循环 10 轮，防止模型反复调工具导致死循环
    for _ in 0..10 {
        // 检查点：每轮开始前先看用户有没有点「停止」
        // 放在这里是因为工具执行完可能还要跑好几秒，那段时间里用户改主意了
        if cancel.is_cancelled() {
            // 推取消事件，前端把「生成中」收掉
            let _ = tx.send(StreamEvent::Cancelled).await;
            // 返回空结果，结束这一趟
            return Ok(ChatResponse {
                // 已经被取消，没有最终答复
                content: String::new(),
                // 没有 token 统计
                usage_tokens: None,
                // 没有工具调用
                tool_calls: Vec::new(),
                // 用 finish_reason 标记「被取消」
                finish_reason: Some("cancelled".to_string()),
            });
        }
        // 发起一轮模型请求，流式事件会经 tx -> rx -> 前端
        // 这一句是整个流程里唯一真正「说话」的地方，文字和工具调用都在这里产生
        // cancel 传下去：网络读取过程中也能被用户打断
        let result = provider.chat_stream(&req, tx.clone(), cancel.clone()).await?;

        // 3) 分两种结局：模型要工具，或者模型给最终答案
        // 没有工具调用说明模型已经给出最终答复，可以收工
        if result.tool_calls.is_empty() {
            // 推一个总结束事件，整个 agent 流程到此为止
            let _ = tx.send(StreamEvent::Done).await;
            // 返回最终答复
            return Ok(result);
        }

        // 4) 模型点了菜：先把它的这一轮回复原样放回历史
        // 必须放回历史，否则第二轮模型不知道自己点过什么工具
        append_assistant(&mut req.messages, &result);

        // 5) 逐个执行模型点名的工具
        for call in &result.tool_calls {
            // 执行工具前再检查一次：模型点了工具但用户在这期间点了停止
            if cancel.is_cancelled() {
                // 推取消事件
                let _ = tx.send(StreamEvent::Cancelled).await;
                // 返回空结果，结束这一趟
                return Ok(ChatResponse {
                    // 已被取消
                    content: String::new(),
                    // 没有 token 统计
                    usage_tokens: None,
                    // 没有工具调用
                    tool_calls: Vec::new(),
                    // 标记「被取消」
                    finish_reason: Some("cancelled".to_string()),
                });
            }
            // 执行工具；失败时把错误文本当成结果回给模型，让它自己决定怎么办
            // （工具失败也是一条有效信息，模型可以换个思路重试，所以不中断循环）
            let output = registry
                .execute(call)
                .await
                .unwrap_or_else(|e| format!("工具执行失败: {}", e));
            // 把工具结果作为一条 role=tool 的消息追加进历史
            // tool_call_id 必填，它是模型把结果和刚才那次调用对起来的唯一凭据
            append_tool_result(&mut req.messages, call, output);
        }
        // 循环回到开头，把带上工具结果的历史再发给模型
    }

    // 6) 走满 10 轮还没结束：属于异常情况，但也要让前端知道结束了
    // 走到这里说明超过轮数上限，仍然要收尾
    let _ = tx.send(StreamEvent::Done).await;
    // 返回明确错误，不静默返回空结果
    Err(anyhow::anyhow!("工具调用轮数超过上限 10"))
}
