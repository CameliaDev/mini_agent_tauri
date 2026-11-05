// 消息区：滚动容器 + 消息行（参考 shadcn 的 Message / Bubble 划分）
import { useMemo, useState } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { activeMessages, useChatStore, type UiMessage } from "../store";
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
            if (message.role === "tool") {
              return <ToolCard key={message.id} message={message} />;
            }

            // 助手消息：左侧小头像 + 正文，不加气泡
            if (message.role === "assistant") {
              const waiting =
                message.phase === "waiting" ||
                (message.streaming && !message.content);
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
                    {waiting && <span className="shimmer">思考中…</span>}
                    {message.phase === "streaming" && message.content && (
                      <span className="caret" />
                    )}
                    {message.phase === "streaming" && (
                      <div className="msg__status">生成中</div>
                    )}
                    {message.phase === "stopped" && (
                      <div className="msg__status">已停止</div>
                    )}
                    {message.phase === "failed" && (
                      <div className="msg__status msg__status--bad">
                        生成失败
                      </div>
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

const TOOL_STATUS_LABEL = {
  running: "运行中",
  done: "完成",
  error: "失败",
  stopped: "已停止",
} as const;

// 工具卡片：名字 + 状态。结束后输出默认折叠
function ToolCard({ message }: { message: UiMessage }) {
  const [open, setOpen] = useState(false);
  const status = message.toolStatus ?? "done";
  const canExpand = status !== "running" && message.content.length > 0;
  return (
    <div className={`toolcard toolcard--${status}`}>
      <div className="toolcard__row">
        <code className="toolcard__name">{message.toolName ?? "未知工具"}</code>
        <span className="toolcard__status">{TOOL_STATUS_LABEL[status]}</span>
        {canExpand && (
          <button
            type="button"
            className="toolcard__toggle"
            onClick={() => setOpen((value) => !value)}
          >
            {open ? "收起" : "展开"}
          </button>
        )}
      </div>
      {open && canExpand && (
        <pre className="toolcard__body">{message.content}</pre>
      )}
    </div>
  );
}
