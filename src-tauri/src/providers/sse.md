# `sse.rs`

- 源码：[`sse.rs`](./sse.rs)
- 分层：模型适配层 / 流式响应解析
- 拆分来源：原 `llm/chat.rs` 中 `chat_stream` 的后半段

## 职责

把厂商返回的 SSE 字节流，变成「文字片段 + 工具调用 + 结束原因」，同时把过程事件推给通道。

## 这个文件刻意分成两半

| 部分 | 内容 | 特点 |
| --- | --- | --- |
| 纯函数 | `ParsedLine`、`ToolCallDelta`、`parse_line`、`parse_tool_call_delta`、`is_done_line`、`merge_tool_call` | 不碰网络、不碰通道；同样的输入永远得到同样的输出，所以能直接写单元测试 |
| 管道 | `parse_response` | 读字节流、切行、调用纯函数、推事件、汇总成 `ChatResponse` |

这是本项目里唯一有单元测试的后端文件，也是新手最值得照着抄的写法：**先把难懂的部分变成纯函数，再让管道去调用它。**

## 包含的定义

| 定义 | 作用 |
| --- | --- |
| `struct ParsedLine` | 一行 SSE 解析出来的内容：文字片段 + 工具调用分片 + 结束原因 |
| `struct ToolCallDelta` | 工具调用的一个分片：`index`、`id`、`name`、`arguments` 都是可选 |
| `fn parse_line(&str) -> Option<ParsedLine>` | 一行文本 → 结构化数据；不是 `data:` 行或 JSON 坏了都返回 `None` |
| `fn parse_tool_call_delta(&Value) -> Option<ToolCallDelta>` | 从 JSON 里取出一个工具调用分片 |
| `fn is_done_line(&str) -> bool` | 判断是不是结束标记 `[DONE]` |
| `fn merge_tool_call(&mut Vec<ToolCall>, &ToolCallDelta)` | 把分片按 `index` 合并进累积列表 |
| `struct ByteBuffer` | 字节缓冲区：网络分包可能把一行 JSON 切成两半，先攒够一整行再解码 |
| `fn parse_response(Response, &Sender<StreamEvent>, CancelSignal)` | 整个管道：读流、拼行、推事件、返回汇总 |

## 逻辑要点

1. **缓冲区是「字节」不是「字符串」**：一个汉字 3 个字节，被 TCP 切在两个包里时，如果对每个包单独解码，半个汉字会被替换成 U+FFFD（乱码）。所以按字节攒到出现 `'\n'` 才算一行，再整行 `from_utf8`。这是本文件最重要的一次修复。
2. 每行交给 `parse_line`：只有 `data:` 行会被处理，`event:`、注释行、空行直接忽略。
3. 文字片段（`delta.content`）拼进 `full`，同时推一个 `TextDelta` 事件。
4. 工具调用分片（`delta.tool_calls`）交给 `merge_tool_call`：`id` 和工具名来了就覆盖，**参数必须追加**，因为它是一段段送来的。
5. 结束时若累积到工具调用，推一个 `ToolCalls` 事件；最后返回 `ChatResponse`。
6. **读流时同时盯着取消信号**：`tokio::select!` 让「来数据」和「用户点了停止」赛跑，被取消时跳出循环、推 `StreamEvent::Cancelled`、把已收到的半截文字作为结果返回。

## 单元测试

文件末尾有 5 个测试，`cd src-tauri && cargo test` 就能跑：

| 测试 | 验证什么 |
| --- | --- |
| `parses_text_delta` | 普通文字帧能解析出文字 |
| `ignores_non_data_lines` | 事件行、注释行、空行、坏 JSON 都被忽略 |
| `merges_split_tool_call_arguments` | 参数分片按顺序拼成完整 JSON |
| `detects_done_marker` | `[DONE]` 识别（带空格和不带空格都认） |
| `reads_finish_reason` | 结束原因能被读出来 |
| `keeps_chinese_intact_across_chunks` | **一个汉字被 TCP 切在两半时不能变成乱码**（防回归） |

## 依赖关系

- 依赖：[`agent/events.rs`](../agent/events.md) 的 `StreamEvent`、[`agent/cancel.rs`](../agent/cancel.md) 的 `CancelSignal`、[`protocol.rs`](../protocol.md) 的 `ChatResponse` / `ToolCall`、`futures_util::StreamExt`。
- 被谁使用：[`openai_compat.rs`](./openai_compat.md) 的 `chat_stream`。

## 当前行为说明

- 按单个 `data:` 行解析，未合并同一事件里的多行 `data`；解析失败的行被静默跳过。
- 缓冲区没有上限：厂商发来一条超长行时内存会一直涨（现实中 SSE 行都很短，暂时不设限）。
- 取消信号被触发时，正在等待的 `stream.next()` 会被丢弃——这正是我们要的效果（打断网络读取）。
- `call["index"]` 没有上限，会按该值把累积列表补到对应长度。
- `finish_reason` 采集了但主循环目前不读它；`usage_tokens` 恒为 `None`。
