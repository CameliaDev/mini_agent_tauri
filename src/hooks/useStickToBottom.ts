// ============================================================================
// 流式滚动锚定：让消息区在「用户想看底部」时自动跟随，在「用户往上翻」时别抢
//
// 问题：流式输出时每个字都会追加到消息末尾，如果无脑 scrollTop = scrollHeight，
//       用户想回头看之前的内容时会被硬生生拽回底部，很烦。
//
// 做法（业界通行方案，ChatGPT / shadcn-chatbot-kit 都是这个思路）：
//   1. 监听容器的 scroll 事件，算出「离底部还有多少像素」
//   2. 距离小于阈值（比如 80px）就认为用户想看底部 → 标记 shouldStick = true
//   3. 距离大于阈值说明用户往上翻了 → shouldStick = false
//   4. 内容变化时，只有 shouldStick 为 true 才自动滚到底
//   5. 用户往上翻时，右下角显示一个「回到底部」按钮
//
// 关键细节：判断「离底部多远」要在内容变化之后算，因为内容变长时 scrollHeight 也会变。
// ============================================================================

// 引入 React 的钩子
import { useCallback, useEffect, useRef, useState } from "react";

// 阈值：离底部小于这个距离（像素）就认为用户想看底部
const STICK_THRESHOLD = 80;

// 定义返回值
export interface StickToBottom {
  // 挂到消息滚动容器上的 ref
  scrollRef: React.RefObject<HTMLDivElement | null>;
  // 用户是否贴着底部
  isPinned: boolean;
  // 手动滚到底部（点「回到底部」按钮时用）
  scrollToBottom: () => void;
  // 滚动事件的处理函数，挂在容器的 onScroll 上
  onScroll: () => void;
}

// 定义 useStickToBottom 钩子
export function useStickToBottom(
  // 内容变化时传进来（消息条数、流式文字长度等），用来触发自动滚动
  content: unknown,
): StickToBottom {
  // 容器 ref：拿到真实的 DOM 节点才能读 scrollHeight
  const scrollRef = useRef<HTMLDivElement | null>(null);
  // 是否贴着底部
  const [isPinned, setIsPinned] = useState(true);
  // 用 ref 存一份 isPinned：scroll 回调里要读到最新值，
  // 但把 isPinned 放进 useEffect 依赖会让回调每次重建，得不偿失
  const pinnedRef = useRef(true);

  // 滚到底部
  const scrollToBottom = useCallback((behavior: ScrollBehavior = "smooth") => {
    // 拿容器
    const el = scrollRef.current;
    // 容器还没挂上就什么都不做
    if (!el) return;
    // 平滑滚到底
    el.scrollTo({ top: el.scrollHeight, behavior });
    // 标记为已贴底
    pinnedRef.current = true;
    // 同步给 React 状态（用来显示/隐藏「回到底部」按钮）
    setIsPinned(true);
  }, []);

  // 监听滚动：判断用户是贴着底部还是往上翻了
  const onScroll = useCallback(() => {
    // 拿容器
    const el = scrollRef.current;
    // 容器不存在就跳过
    if (!el) return;
    // 离底部的距离 = 总高度 - 当前滚动位置 - 可视高度
    const distance = el.scrollHeight - el.scrollTop - el.clientHeight;
    // 小于阈值就算贴着底部
    const pinned = distance <= STICK_THRESHOLD;
    // 和上次不一样才更新，避免频繁 setState
    if (pinned !== pinnedRef.current) {
      // 更新 ref
      pinnedRef.current = pinned;
      // 更新 React 状态
      setIsPinned(pinned);
    }
  }, []);

  // 内容变化时：只有用户贴着底部才自动跟随
  useEffect(() => {
    // 用户往上翻了，就别动他的滚动位置
    if (!pinnedRef.current) return;
    // 拿容器
    const el = scrollRef.current;
    // 容器还没挂上就跳过
    if (!el) return;
    // 直接赋值 scrollTop：流式输出时每个字都会触发一次，
    // 用 scrollTo 的 smooth 反而会一直排队动画、看起来卡顿
    el.scrollTop = el.scrollHeight;
  }, [content]);

  // 首次挂载时滚到底部（等 DOM 真正渲染完再滚）
  useEffect(() => {
    // 延后一帧，等内容有高度了再滚
    const timer = window.setTimeout(() => {
      // 拿容器
      const el = scrollRef.current;
      // 有容器才滚
      if (el) el.scrollTop = el.scrollHeight;
    }, 0);
    // 组件卸载时清掉定时器
    return () => window.clearTimeout(timer);
  }, []);

  // 返回给组件用
  return {
    // 容器 ref
    scrollRef,
    // 是否贴着底部
    isPinned,
    // 手动滚到底
    scrollToBottom,
    // scroll 事件处理函数
    onScroll,
  };
}
