import assert from "node:assert/strict";
import { test } from "node:test";
import {
  __blockWrites,
  __messages,
  __resetDb,
  __writesStarted,
  invoke,
} from "@tauri-apps/api/core";
import {
  activeMessages,
  applyIncomingMessages,
  useChatStore,
  type Session,
  type UiMessage,
} from "../src/store.ts";
import { hydrateFromDb, loadMessagesOf, persistSession } from "../src/ipc/sessions.ts";

function boot(): void {
  __resetDb();
  const session: Session = {
    id: "boot",
    title: "新对话",
    messages: [],
    createdAt: 1,
    updatedAt: 1,
    messagesLoaded: true,
  };
  useChatStore.setState({
    sessions: [session],
    activeSessionId: session.id,
    isStreaming: false,
    stopRequested: false,
    error: null,
    draft: "",
  });
}

function user(id: string, content: string): UiMessage {
  return { id, role: "user", content, createdAt: 1 };
}

async function seedSession(
  id: string,
  title: string,
  updatedAt: number,
  contents: string[],
): Promise<void> {
  await invoke("save_session", {
    session: { id, title, created_at: 1, updated_at: updatedAt },
  });
  for (const [index, content] of contents.entries()) {
    await invoke("append_message", {
      sessionId: id,
      seq: index,
      message: {
        id: `${id}-${index}`,
        role: "user",
        content,
        tool_name: null,
        tool_call: null,
        created_at: index + 1,
      },
    });
  }
}

test("switching away and back keeps a message that is not in the database yet", async () => {
  boot();
  await seedSession("a", "旧对话", 50, ["旧消息"]);
  await seedSession("b", "另一个", 20, []);
  await hydrateFromDb();

  assert.equal(useChatStore.getState().activeSessionId, "a");
  useChatStore.getState().appendUser("新消息");
  useChatStore.getState().selectSession("b");
  await loadMessagesOf("b");
  useChatStore.getState().selectSession("a");
  await loadMessagesOf("a");

  assert.deepEqual(
    activeMessages(useChatStore.getState()).map((message) => message.content),
    ["旧消息", "新消息"],
  );
  const other = useChatStore.getState().sessions.find((session) => session.id === "b");
  assert.deepEqual(other?.messages ?? [], []);
});

test("a new session does not take or clear another session's messages", async () => {
  boot();
  await seedSession("a", "甲", 50, ["旧消息"]);
  await hydrateFromDb();
  useChatStore.getState().appendUser("留在甲");
  const idA = useChatStore.getState().activeSessionId;

  useChatStore.getState().newSession();
  const createdId = useChatStore.getState().activeSessionId;
  await persistSession(idA);
  await persistSession(createdId);

  useChatStore.getState().selectSession(idA);
  await loadMessagesOf(idA);
  assert.deepEqual(
    activeMessages(useChatStore.getState()).map((message) => message.content),
    ["旧消息", "留在甲"],
  );

  useChatStore.getState().selectSession(createdId);
  await loadMessagesOf(createdId);
  assert.deepEqual(activeMessages(useChatStore.getState()), []);
  const original = useChatStore.getState().sessions.find((session) => session.id === idA);
  assert.deepEqual(
    original?.messages.map((message) => message.content),
    ["旧消息", "留在甲"],
  );
});

test("a transcript that was saved loads again after the memory copy is dropped", async () => {
  boot();
  await seedSession("a", "甲", 50, ["旧消息"]);
  await hydrateFromDb();
  useChatStore.getState().appendUser("要落库");
  const id = useChatStore.getState().activeSessionId;
  await persistSession(id);

  useChatStore.setState((state) => ({
    sessions: state.sessions.map((session) =>
      session.id === id
        ? { ...session, messages: [], messagesLoaded: false }
        : session,
    ),
  }));
  await loadMessagesOf(id);
  assert.deepEqual(
    activeMessages(useChatStore.getState()).map((message) => message.content),
    ["旧消息", "要落库"],
  );
});

test("messages typed before the first load are kept and then saved in order", async () => {
  boot();
  await seedSession("a", "甲", 50, ["旧消息"]);
  useChatStore.setState({
    sessions: [
      {
        id: "a",
        title: "甲",
        messages: [user("new", "刚发的")],
        createdAt: 1,
        updatedAt: 60,
        messagesLoaded: false,
      },
    ],
    activeSessionId: "a",
  });

  await loadMessagesOf("a");
  assert.deepEqual(
    activeMessages(useChatStore.getState()).map((message) => message.content),
    ["旧消息", "刚发的"],
  );
  assert.deepEqual(
    __messages("a").map((message) => [message.seq, message.content]),
    [
      [0, "旧消息"],
      [1, "刚发的"],
    ],
  );
});

test("an older persist cannot overwrite a newer message body", async () => {
  boot();
  useChatStore.getState().appendUser("短");
  const id = useChatStore.getState().activeSessionId;
  const hold = __blockWrites();
  const first = persistSession(id);
  for (let turn = 0; turn < 50 && __writesStarted() < 1; turn += 1) {
    await new Promise((resolve) => setTimeout(resolve, 0));
  }
  assert.ok(__writesStarted() >= 1);
  useChatStore.setState((state) => ({
    sessions: state.sessions.map((session) =>
      session.id === id
        ? {
            ...session,
            messages: session.messages.map((message) => ({
              ...message,
              content: "短很长",
            })),
          }
        : session,
    ),
  }));
  const second = persistSession(id);
  hold.release();
  await first;
  await second;
  assert.deepEqual(
    __messages(id).map((message) => message.content),
    ["短很长"],
  );
});

test("streaming text keeps arriving on the session that sent it", async () => {
  boot();
  await seedSession("a", "甲", 50, ["问题"]);
  await seedSession("b", "乙", 20, []);
  await hydrateFromDb();
  const assistantId = useChatStore.getState().startAssistant();
  useChatStore.getState().selectSession("b");
  await loadMessagesOf("b");
  useChatStore.getState().appendDelta(assistantId, "还在甲");
  useChatStore.getState().selectSession("a");
  await loadMessagesOf("a");
  assert.equal(
    activeMessages(useChatStore.getState()).some((message) =>
      message.content.includes("还在甲"),
    ),
    true,
  );
});

test("applyIncomingMessages does not replace a transcript the session already owns", () => {
  const session: Session = {
    id: "a",
    title: "甲",
    messages: [user("1", "活的")],
    createdAt: 1,
    updatedAt: 1,
    messagesLoaded: true,
  };
  assert.equal(applyIncomingMessages(session, []), session);
});

test("applyIncomingMessages keeps local text that is ahead of the database row", () => {
  const session: Session = {
    id: "a",
    title: "甲",
    messages: [
      {
        id: "1",
        role: "assistant",
        content: "hello world",
        streaming: true,
        phase: "streaming",
      },
    ],
    createdAt: 1,
    updatedAt: 1,
    messagesLoaded: false,
  };
  const merged = applyIncomingMessages(session, [
    { id: "1", role: "assistant", content: "hello" },
  ]);
  assert.equal(merged.messages[0].content, "hello world");
  assert.equal(merged.messagesLoaded, true);
});

test("a cleared session ignores a late database snapshot", () => {
  boot();
  useChatStore.getState().appendUser("会被清掉");
  useChatStore.getState().clearActiveSession();
  useChatStore.getState().setSessionMessages("boot", [user("old", "不该回来")]);
  assert.deepEqual(activeMessages(useChatStore.getState()), []);
});
