export type PageId = "market" | "watch" | "limit" | "theme" | "journal";
export type SourceContext = {
  date: string;
  sector: string;
  codes: string[];
  reason?: string;
  origin: PageId;
};
export type DraftRequest = SourceContext & { requestId: string };
export type WatchGroup = {
  id: string;
  name: string;
  codes: string[];
  color: string;
  note: string;
};
export const pageNames: Record<PageId, string> = {
  market: "行情",
  watch: "盯盘",
  limit: "涨停",
  theme: "题材",
  journal: "复盘",
};
export const DEMO_DATE = "2026-10-14";
export const initialWatchGroups: WatchGroup[] = [
  {
    id: "media",
    name: "传媒 · 情绪观察",
    codes: ["603598", "001318", "300059"],
    color: "#009b9a",
    note: "关注核心标的承接与板块跟随",
  },
  {
    id: "technology",
    name: "科技 · 趋势跟踪",
    codes: ["688981", "002475", "300750"],
    color: "#6686c0",
    note: "观察放量与均线支撑",
  },
  {
    id: "auto",
    name: "汽车 · 轮动机会",
    codes: ["601127", "002594", "000333"],
    color: "#ba9466",
    note: "保留分歧后的确认机会",
  },
  {
    id: "consumer",
    name: "消费 · 防守配置",
    codes: ["600519", "000858", "600036"],
    color: "#9b83ad",
    note: "低波动观察，不追涨",
  },
];
export type PageActions = {
  onInspect: (code: string, context?: SourceContext) => void;
  onOpenMarket: (code: string) => void;
  onAddToWatch: (context: SourceContext) => void;
  onCreateDraft: (context: SourceContext) => void;
  onNotice: (message: string) => void;
};
