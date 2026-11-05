// 左侧会话栏：新建对话 + 会话列表（按今天 / 昨天 / 更早分组）
import { useMemo } from "react";
import { groupOf, useChatStore } from "../store";
import type { Session } from "../store";
// 引入持久化层的三个函数：切换时读消息、删除时删库、新建时落库
import { loadMessagesOf, persistSession, removeSession } from "../ipc/sessions";

export function Sidebar() {
  const sessions = useChatStore((s) => s.sessions);
  const activeSessionId = useChatStore((s) => s.activeSessionId);
  const newSession = useChatStore((s) => s.newSession);
  const selectSession = useChatStore((s) => s.selectSession);
  const deleteSession = useChatStore((s) => s.deleteSession);

  // 切换会话：先切内存，再从数据库把消息读回来
  // 顺序很重要：先切，loadMessagesOf 才知道往哪个会话里填
  async function openSession(id: string) {
    // 内存里立刻切过去，界面马上响应
    selectSession(id);
    // 再从数据库读这个会话的消息（慢一点也没关系，界面已经在新会话上了）
    await loadMessagesOf(id);
  }

  // 删除会话：内存里删，数据库里也删
  async function remove(id: string) {
    // 先删数据库那边的（消息随外键级联一起删）
    await removeSession(id);
    // 再删内存里的
    deleteSession(id);
  }

  // 新建对话：建完立刻落库，这样刷新页面它还在
  async function create() {
    // 内存里新建（会切过去）
    newSession();
    // 拿到当前会话 id
    const state = useChatStore.getState();
    // 落库
    await persistSession(state.activeSessionId);
  }

  // 分组：先按时间分桶，桶内保持原有顺序（新的在前）
  const groups = useMemo(() => {
    const map = new Map<string, Session[]>();
    for (const session of sessions) {
      const key = groupOf(session);
      map.set(key, [...(map.get(key) ?? []), session]);
    }
    return [...map.entries()];
  }, [sessions]);

  return (
    <aside className="sidebar">
      <div className="sidebar__head">
        <button type="button" className="sidebar__new" onClick={() => void create()}>
          <span className="sidebar__plus" aria-hidden="true">
            +
          </span>
          新建对话
        </button>
      </div>

      <nav className="sidebar__list">
        {groups.map(([label, items]) => (
          <section key={label} className="sidebar__group">
            <h2 className="sidebar__group-title">{label}</h2>
            {items.map((session) => (
              <div
                key={session.id}
                className={`sidebar__row${
                  session.id === activeSessionId ? " sidebar__row--active" : ""
                }`}
              >
                <button
                  type="button"
                  className="sidebar__item"
                  onClick={() => void openSession(session.id)}
                  title={session.title}
                >
                  {session.title}
                </button>
                <button
                  type="button"
                  className="sidebar__del"
                  aria-label="删除会话"
                  onClick={() => void remove(session.id)}
                >
                  ×
                </button>
              </div>
            ))}
          </section>
        ))}
      </nav>

      <footer className="sidebar__foot">
        <span className="sidebar__status" aria-hidden="true" />
        DeepSeek · deepseek-chat
      </footer>
    </aside>
  );
}
