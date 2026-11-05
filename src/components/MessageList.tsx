// 消息区：滚动容器 + 消息行（参考 shadcn 的 Message / Bubble / Marker 划分）
import { useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { activeMessages, useChatStore } from "../store";
import { useStickToBottom } from "../hooks/useStickToBottom";
import { EmptyState } from "./EmptyState";
import { regenerate } from "../ipc/chat";

export function MessageList() {
  // 只订阅当前会话里的消息
  const messages = useChatStore((s) => activeMessages(s));
  const isStreaming = useChatStore((s) => s.isStreaming);
  const setDraft = useChatStore((s) => s.setDraft);

  // 滚动锚定：内容变化时如果用户贴着底部就自动跟随
  // 用消息条数 + 最后一条的长度当依赖，流式输出时长度一直在变
  const lastLength = messages.length
    ? messages[messages.length - 1].content.length
    : 0;
  const { scrollRef, isPinned, scrollToBottom, onScroll } = useStickToBottom(
    // 依赖值：条数 + 末条长度 + 是否在生成
    `${messages.length}-${lastLength}-${isStreaming}`,
  );

  // 最后一条助手消息的 id：只在它下面显示「重新生成」
  const lastAssistantId = useMemo(() => {
    // 从后往前找
    for (let i = messages.length - 1; i >= 0; i -= 1) {
      // 找到助手消息就返回它的 id
      if (messages[i].role === "assistant") return messages[i].id;
    }
    // 没有助手消息
    return null;
  }, [messages]);

  return (
    <div className="messages-wrap">
      <div className="messages" ref={scrollRef} onScroll={onScroll}>
        {messages.length === 0 ? (
          <EmptyState onPick={setDraft} />
        ) : (
          messages.map((message) => {
            // 工具调用：做成一条 Marker 行（带竖线的小行），不进气泡
            if (message.role === "tool") {
              return (
                <div key={message.id} className="marker">
                  <span className="marker__badge">工具</span>
                  <code className="marker__name">
                    {message.toolName ?? "未知工具"}
                  </code>
                  <span className="marker__args">
                    {summarize(message.toolCall?.function.arguments)}
                  </span>
                </div>
              );
            }

            // 助手消息：左侧小头像 + 正文，不加气泡（成熟产品的排版方式）
            if (message.role === "assistant") {
              return (
                <article key={message.id} className="msg msg--assistant">
                  <div className="msg__avatar" aria-hidden="true">
                    M
                  </div>
                  <div className="msg__body">
                    {message.content && (
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {message.content}
                      </ReactMarkdown>
                    )}
                    {/* 等待首个字符：微光文字（参考 shadcn 的 shimmer） */}
                    {message.streaming && !message.content && (
                      <span className="shimmer">正在思考…</span>
                    )}
                    {/* 正在输出：末尾跟一个闪烁光标 */}
                    {message.streaming && message.content && (
                      <span className="caret" />
                    )}
                    {/* 最后一条助手回复下面给一个「重新生成」 */}
                    {message.id === lastAssistantId && !message.streaming && (
                      <div className="msg__actions">
                        <button
                          type="button"
                          className="msg__action"
                          onClick={() => void regenerate()}
                        >
                          重新生成
                        </button>
                      </div>
                    )}
                  </div>
                </article>
              );
            }

            // 用户消息：右对齐的浅色气泡
            return (
              <article key={message.id} className="msg msg--user">
                <div className="msg__body">
                  <div className="bubble bubble--user">{message.content}</div>
                </div>
              </article>
            );
          })
        )}
      </div>

      {/* 用户往上翻了才出现：点一下回到底部 */}
      {!isPinned && (
        <button
          type="button"
          className="jump-bottom"
          onClick={() => scrollToBottom()}
          aria-label="回到底部"
        >
          <span aria-hidden="true">↓</span>
        </button>
      )}
    </div>
  );
}

// 把工具参数压成一行摘要
function summarize(args?: string): string {
  if (!args) return "";
  const oneLine = args.replace(/\s+/g, " ").trim();
  return oneLine.length > 60 ? `${oneLine.slice(0, 60)}…` : oneLine;
}
