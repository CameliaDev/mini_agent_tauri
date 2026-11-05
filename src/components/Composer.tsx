// 底部输入区：一张居中的卡片（输入框 + 提示 + 发送/停止按钮）
import type { KeyboardEvent } from "react";
import { sendMessage, stopGeneration } from "../ipc/chat";
import { useChatStore } from "../store";

export function Composer() {
  // 输入内容放在 store 里，空态示例按钮才能往同一个输入框里塞字
  const draft = useChatStore((s) => s.draft);
  const setDraft = useChatStore((s) => s.setDraft);
  const isStreaming = useChatStore((s) => s.isStreaming);

  function submit() {
    // 去掉首尾空格；空内容或正在生成都不发
    const value = draft.trim();
    if (!value || isStreaming) return;
    setDraft("");
    void sendMessage(value);
  }

  function onKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    // Enter 发送，Shift+Enter 换行
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      submit();
    }
  }

  return (
    <div className="composer">
      <form
        className="composer__box"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <textarea
          className="composer__input"
          value={draft}
          onChange={(event) => setDraft(event.currentTarget.value)}
          onKeyDown={onKeyDown}
          placeholder="给 mini_agent 发消息…"
          rows={2}
        />
        <div className="composer__foot">
          <span className="composer__hint">
            Enter 发送 · Shift+Enter 换行
          </span>
          <div className="composer__actions">
            {/* 生成中：换成「停止」按钮，点一下打断后端 */}
            {isStreaming ? (
              <button
                type="button"
                className="composer__stop"
                onClick={() => void stopGeneration()}
              >
                停止
              </button>
            ) : (
              <button
                type="submit"
                className="composer__send"
                disabled={draft.trim().length === 0}
              >
                发送
              </button>
            )}
          </div>
        </div>
      </form>
    </div>
  );
}
