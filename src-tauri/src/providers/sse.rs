// ============================================================================
// 这一层负责：把厂商返回的 SSE 字节流，变成「文字片段 + 工具调用 + 结束原因」
//
// 写法上刻意分成两半，方便学习和测试：
//   1) 纯函数部分：parse_line / parse_tool_call_delta / merge_tool_call
//      —— 不碰网络、不碰通道，同样的输入永远得到同样的输出，可以直接写单元测试
//   2) 管道部分：parse_response
//      —— 负责读字节流、切行、把纯函数的结果推给通道
//
// 两个关键设计（都是踩过坑才补上的）：
//   A. 缓冲区是「字节」不是「字符串」
//      一个汉字 3 个字节，被 TCP 切在两个包里时，如果对每个包单独解码，
//      半个汉字会被替换成 U+FFFD（乱码）。所以必须按字节攒够一整行再解码。
//   B. 读流时同时盯着「取消信号」
//      用 tokio::select! 让「来数据」和「用户点了停止」赛跑，谁先发生就走哪条路。
// ============================================================================

// 引入 StreamExt，提供 .next() 方法逐个读取网络数据块
use futures_util::StreamExt;
// 引入 Tauri 异步运行时重导出的通道发送端
use tauri::async_runtime::Sender;
// 引入统一事件类型
use crate::agent::events::StreamEvent;
// 引入取消信号
use crate::agent::cancel::CancelSignal;
// 引入响应以及工具调用相关的数据结构
use crate::protocol::{ChatResponse, ToolCall, ToolCallFunction};

// ============================================================================
// 第一部分：纯函数
// ============================================================================

// 一行 SSE 文本解析出来的内容
#[derive(Debug, Default, PartialEq)]
struct ParsedLine {
    // 文字片段；这一帧没有文字就是 None
    text: Option<String>,
    // 工具调用分片；这一帧没有工具调用就是空数组
    tool_calls: Vec<ToolCallDelta>,
    // 结束原因；只有最后一帧才有，例如 "stop" 或 "tool_calls"
    finish_reason: Option<String>,
}

// 工具调用的一个分片：模型每次只给一小块，需要按顺序拼起来
#[derive(Debug, PartialEq)]
struct ToolCallDelta {
    // 第几个工具调用；并行调用多个工具时靠它区分
    index: usize,
    // 调用编号，只有第一帧有
    id: Option<String>,
    // 工具名，只有第一帧有
    name: Option<String>,
    // 参数片段，每次一小段，收到就追加
    arguments: Option<String>,
}

// 把一行 SSE 文本解析成结构化数据
// 纯函数：不碰网络、不碰通道，所以好测、好改
fn parse_line(line: &str) -> Option<ParsedLine> {
    // 去掉首尾空白，顺带处理 Windows 的 \r\n
    let line = line.trim();
    // SSE 里除了 data: 还有 event:、id:、以冒号开头的注释行和空行，这里只关心 data:
    if !line.starts_with("data:") {
        // 不是数据行，返回 None 表示「这行不用管」
        return None;
    }
    // 剥掉 "data:" 这 5 个字符，剩下的就是 JSON 文本
    let data = line[5..].trim();
    // 空数据行直接忽略
    if data.is_empty() {
        return None;
    }
    // 把 JSON 文本解析成 Value；解析不了就当这行不存在
    let value: serde_json::Value = match serde_json::from_str(data) {
        // 解析成功
        Ok(v) => v,
        // 解析失败，跳过这一行
        Err(_) => return None,
    };
    // 取 choices[0].delta.content；字段不存在时得到 Null，as_str() 会给出 None
    let text = value["choices"][0]["delta"]["content"]
        .as_str()
        // 空字符串片段没有意义，过滤掉
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    // 取本帧的工具调用数组；普通文字帧里这里是 Null，as_array() 会给出 None
    let tool_calls = value["choices"][0]["delta"]["tool_calls"]
        .as_array()
        .map(|items| items.iter().filter_map(parse_tool_call_delta).collect())
        .unwrap_or_default();
    // 取结束原因；非结束帧是 null
    let finish_reason = value["choices"][0]["finish_reason"]
        .as_str()
        .map(|s| s.to_string());
    // 返回这一行的解析结果
    Some(ParsedLine {
        text,
        tool_calls,
        finish_reason,
    })
}

// 从 JSON 里的一个元素取出「工具调用分片」
fn parse_tool_call_delta(item: &serde_json::Value) -> Option<ToolCallDelta> {
    // index 缺失时当成 0
    let index = item["index"].as_u64().unwrap_or(0) as usize;
    // 三个字段都可能这一帧没给，所以都是 Option
    Some(ToolCallDelta {
        // 第几个工具调用
        index,
        // 调用编号
        id: item["id"].as_str().map(|s| s.to_string()),
        // 工具名
        name: item["function"]["name"].as_str().map(|s| s.to_string()),
        // 参数片段
        arguments: item["function"]["arguments"]
            .as_str()
            .map(|s| s.to_string()),
    })
}

// 判断这一行是不是结束标记 [DONE]
fn is_done_line(line: &str) -> bool {
    // 先剥掉 data: 前缀，再和 [DONE] 比较
    line.trim()
        .strip_prefix("data:")
        .map(|rest| rest.trim() == "[DONE]")
        .unwrap_or(false)
}

// 把一个分片合并进累积列表
// 规则：id 和工具名只出现在第一帧，来了就覆盖；参数是分片，必须追加
fn merge_tool_call(pending: &mut Vec<ToolCall>, delta: &ToolCallDelta) {
    // 列表里还没这个位置，就先补一个空订单占位
    while pending.len() <= delta.index {
        // 占位订单：id、工具名、参数都先留空，后面几帧往里填
        pending.push(ToolCall {
            // 调用编号待填
            id: String::new(),
            // 协议固定值
            kind: "function".to_string(),
            // 工具名和参数待填
            function: ToolCallFunction {
                // 工具名待填
                name: String::new(),
                // 参数待拼
                arguments: String::new(),
            },
        });
    }
    // 拿到这个位置的订单
    let target = &mut pending[delta.index];
    // 调用编号：来了就覆盖
    if let Some(id) = &delta.id {
        target.id = id.clone();
    }
    // 工具名：来了就覆盖
    if let Some(name) = &delta.name {
        target.function.name = name.clone();
    }
    // 参数片段：必须追加，不能覆盖，否则后面的片段会丢掉前面的内容
    if let Some(args) = &delta.arguments {
        target.function.arguments.push_str(args);
    }
}

// ============================================================================
// 第二部分：字节缓冲（把「网络字节流」变成「一行行完整文本」）
// ============================================================================

// 字节缓冲区：网络分包可能把一行 JSON 切成两半，先攒够一整行再解码
#[derive(Debug, Default)]
struct ByteBuffer {
    // 还没凑够一行的字节
    buf: Vec<u8>,
}

impl ByteBuffer {
    // 追加一段网络字节
    fn push(&mut self, chunk: &[u8]) {
        // 直接接到尾部
        self.buf.extend_from_slice(chunk);
    }

    // 取出下一个完整的行（不含换行符）；没有完整行返回 None
    // 关键点：只有「一整行字节」才解码，中文不会被腰斩成乱码
    fn take_line(&mut self) -> Option<String> {
        // 找换行符的位置
        let pos = self.buf.iter().position(|b| *b == b'\n')?;
        // 把这一行（含换行符）从缓冲区里摘走
        let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
        // 去掉末尾的换行符
        line.pop();
        // 去掉 Windows 的 \r
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        // 整行解码：此时字节是完整的，汉字不会被替换成乱码
        Some(String::from_utf8_lossy(&line).into_owned())
    }
}

// ============================================================================
// 第三部分：管道（读字节流、切行、推事件、汇总结果）
// ============================================================================

// 解析厂商返回的 SSE 字节流，边解析边把事件推给通道，最后汇总成本轮结果
pub async fn parse_response(
    // 已经通过 error_for_status 检查过的响应
    resp: reqwest::Response,
    // 借用发送端，解析过程中逐条推送事件
    tx: &Sender<StreamEvent>,
    // 借用取消信号：用户点「停止」时用它打断网络读取
    mut cancel: CancelSignal,
) -> anyhow::Result<ChatResponse> {
    // 把响应体转成一个异步字节流
    let mut stream = resp.bytes_stream();
    // 字节缓冲区：网络分包可能把一行 JSON 切成两半
    let mut buffer = ByteBuffer::default();
    // 累积器：把所有文字片段拼成最终完整回复
    let mut full = String::new();
    // 累积器：流式下工具参数是一段段到达的，要按 index 拼起来
    let mut pending_calls: Vec<ToolCall> = Vec::new();
    // 记录模型给出的结束原因，交给上层决定要不要继续循环
    let mut finish_reason: Option<String> = None;
    // 标记是不是被用户取消了
    let mut cancelled = false;
    // 带标签的外层循环：遇到 [DONE] 或取消时可以直接跳到统一收尾
    'outer: loop {
        // 关键：同时等两件事 —— 「来数据」和「被取消」，谁先发生算谁
        let chunk = tokio::select! {
            // 分支一：用户点了停止
            // wait_cancelled 返回 true 表示确实收到了取消；false 表示发送端被销毁
            // （应用正在退出），两种情况都当作「这一轮到此为止」
            _ = cancel.wait_cancelled() => {
                // 记下被取消了
                cancelled = true;
                // 跳出读取循环，走下面的收尾逻辑（保留已收到的文字）
                break 'outer;
            }
            // 分支二：又收到一块网络数据
            next = stream.next() => {
                // 流结束（None）时跳出；出错时向上传播
                match next {
                    // 拿到一块数据
                    Some(Ok(chunk)) => chunk,
                    // 网络错误：直接返回错误
                    Some(Err(e)) => return Err(e.into()),
                    // 流正常结束
                    None => break 'outer,
                }
            }
        };
        // 把这块字节追加进缓冲区
        buffer.push(&chunk);
        // 只要缓冲区里还有完整的一行就继续处理
        while let Some(line) = buffer.take_line() {
            // 收到 [DONE] 表示厂商已结束本次流
            if is_done_line(&line) {
                // 跳出整个读取循环，走到下面统一的收尾逻辑
                break 'outer;
            }
            // 交给纯函数解析这一行；不是数据行会得到 None
            let Some(parsed) = parse_line(&line) else {
                // 这行不用管，继续下一行
                continue;
            };
            // 有文字片段：拼进完整回复，并通过通道推给上层
            if let Some(text) = parsed.text {
                // 完整回复累加
                full.push_str(&text);
                // 推一个文字事件；上层会转给前端
                if tx.send(StreamEvent::TextDelta(text)).await.is_err() {
                    // 接收端已关闭，用 anyhow 造一个错误返回
                    return Err(anyhow::anyhow!("流式通道已关闭"));
                }
            }
            // 有工具调用分片：逐个合并进累积列表
            for delta in &parsed.tool_calls {
                // 把这一片拼到对应的第几个订单上
                merge_tool_call(&mut pending_calls, delta);
            }
            // 有结束原因就记下来（只有最后一帧有）
            if parsed.finish_reason.is_some() {
                finish_reason = parsed.finish_reason;
            }
        }
    }
    // 被取消了：告诉上层「这是用户主动停的」，不是出错
    if cancelled {
        // 推取消事件；上层收到后会把界面上的「生成中」收掉
        let _ = tx.send(StreamEvent::Cancelled).await;
        // 返回已经收到的半截内容
        return Ok(ChatResponse {
            // 已收到的文字
            content: full,
            // 暂不统计 token
            usage_tokens: None,
            // 取消时不会执行工具
            tool_calls: Vec::new(),
            // 用 finish_reason 标记「被取消」
            finish_reason: Some("cancelled".to_string()),
        });
    }
    // 如果累积到了工具调用，先通过通道推给上层
    if !pending_calls.is_empty() {
        // 推 ToolCalls 事件，上层拿到后去执行工具并发起下一轮
        if tx
            .send(StreamEvent::ToolCalls(pending_calls.clone()))
            .await
            .is_err()
        {
            // 接收端已关闭，不静默吞掉，直接报错
            return Err(anyhow::anyhow!("流式通道已关闭"));
        }
    }
    // 返回本轮汇总，[DONE] 和自然结束两种情况都走这里
    Ok(ChatResponse {
        // 完整文字
        content: full,
        // 暂不统计 token
        usage_tokens: None,
        // 把订单交给上层（agent loop 会用到）
        tool_calls: pending_calls,
        // 把结束原因交给上层
        finish_reason,
    })
}

// ============================================================================
// 单元测试：纯函数部分可以直接测，不需要网络
// 运行：cd src-tauri && cargo test
// ============================================================================
#[cfg(test)]
mod tests {
    // 引入被测的私有函数
    use super::*;

    // 一行普通文字帧应该解析出文字
    #[test]
    fn parses_text_delta() {
        // 构造一行最小的 SSE 文字数据
        let line = r#"data: {"choices":[{"delta":{"content":"你好"}}]}"#;
        // 解析
        let parsed = parse_line(line).expect("这一行应该能解析");
        // 文字应该是「你好」
        assert_eq!(parsed.text.as_deref(), Some("你好"));
        // 没有工具调用
        assert!(parsed.tool_calls.is_empty());
        // 没有结束原因
        assert_eq!(parsed.finish_reason, None);
    }

    // 不是 data: 开头的行都应该被忽略
    #[test]
    fn ignores_non_data_lines() {
        // 事件行
        assert!(parse_line("event: message").is_none());
        // 注释行（心跳）
        assert!(parse_line(": keep-alive").is_none());
        // 空行
        assert!(parse_line("").is_none());
        // 坏 JSON
        assert!(parse_line("data: {oops").is_none());
    }

    // 工具调用的参数分片应该按顺序拼起来
    #[test]
    fn merges_split_tool_call_arguments() {
        // 第一帧：给调用编号、工具名和参数的前半段
        let first = parse_line(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"echo","arguments":"{\"text\""}}]}}]}"#,
        )
        .expect("第一帧应该能解析");
        // 第二帧：只给参数的后半段
        let second = parse_line(
            r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":":\"hi\"}"}}]}}]}"#,
        )
        .expect("第二帧应该能解析");
        // 累积列表
        let mut pending: Vec<ToolCall> = Vec::new();
        // 把两帧都合并进去
        for parsed in [first, second] {
            // 逐片合并
            for delta in &parsed.tool_calls {
                // 合并这一片
                merge_tool_call(&mut pending, delta);
            }
        }
        // 应该只有一个订单
        assert_eq!(pending.len(), 1);
        // 编号来自第一帧
        assert_eq!(pending[0].id, "call_1");
        // 工具名来自第一帧
        assert_eq!(pending[0].function.name, "echo");
        // 参数是两段拼起来的完整 JSON
        assert_eq!(pending[0].function.arguments, r#"{"text":"hi"}"#);
    }

    // 结束标记的识别
    #[test]
    fn detects_done_marker() {
        // 带空格
        assert!(is_done_line("data: [DONE]"));
        // 不带空格
        assert!(is_done_line("data:[DONE]"));
        // 普通数据行不是结束标记
        assert!(!is_done_line(r#"data: {"choices":[]}"#));
    }

    // 结束原因应该被读出来
    #[test]
    fn reads_finish_reason() {
        // 最后一帧带 finish_reason
        let parsed = parse_line(r#"data: {"choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#)
            .expect("这一帧应该能解析");
        // 结束原因应该是 tool_calls
        assert_eq!(parsed.finish_reason.as_deref(), Some("tool_calls"));
    }

    // 关键回归测试：一个汉字被 TCP 切在两个包里时，不能变成乱码
    #[test]
    fn keeps_chinese_intact_across_chunks() {
        // 「你好」的 UTF-8 字节，一共 6 个字节
        let bytes = "你好".as_bytes();
        // 故意在第 3 个字节处切开，正好把第一个汉字劈成两半
        let (head, tail) = bytes.split_at(3);
        // 建缓冲区
        let mut buffer = ByteBuffer::default();
        // 先喂前半截
        buffer.push(head);
        // 这时候还没有换行符，取不出完整行
        assert!(buffer.take_line().is_none());
        // 再喂后半截，并且补上换行符
        let mut rest = tail.to_vec();
        rest.push(b'\n');
        buffer.push(&rest);
        // 现在能取出完整行了
        let line = buffer.take_line().expect("应该能取出一行");
        // 内容必须是完整的「你好」，不能出现替换字符
        assert_eq!(line, "你好");
        // 而且不能包含 U+FFFD（乱码替换符）
        assert!(!line.contains('\u{FFFD}'));
    }
}
