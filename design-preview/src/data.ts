export type Stock = {
  code: string;
  name: string;
  price: number;
  change: number;
  market: "SH" | "SZ";
  sector: string;
};
export const stocks: Stock[] = [
  {
    code: "603598",
    name: "引力传媒",
    price: 21.91,
    change: 9.99,
    market: "SH",
    sector: "文化传媒",
  },
  {
    code: "001318",
    name: "阳光乳业",
    price: 15.43,
    change: 5.4,
    market: "SZ",
    sector: "食品饮料",
  },
  {
    code: "300750",
    name: "宁德时代",
    price: 268.8,
    change: 2.36,
    market: "SZ",
    sector: "电池",
  },
  {
    code: "600519",
    name: "贵州茅台",
    price: 1468.8,
    change: -0.65,
    market: "SH",
    sector: "白酒",
  },
  {
    code: "601127",
    name: "赛力斯",
    price: 123.58,
    change: 4.21,
    market: "SH",
    sector: "汽车整车",
  },
  {
    code: "002594",
    name: "比亚迪",
    price: 106.28,
    change: -1.12,
    market: "SZ",
    sector: "汽车整车",
  },
  {
    code: "300059",
    name: "东方财富",
    price: 29.31,
    change: 3.64,
    market: "SZ",
    sector: "证券",
  },
  {
    code: "688981",
    name: "中芯国际",
    price: 98.36,
    change: 1.28,
    market: "SH",
    sector: "半导体",
  },
  {
    code: "000333",
    name: "美的集团",
    price: 78.15,
    change: -0.82,
    market: "SZ",
    sector: "家用电器",
  },
  {
    code: "002475",
    name: "立讯精密",
    price: 53.96,
    change: 2.12,
    market: "SZ",
    sector: "消费电子",
  },
  {
    code: "600036",
    name: "招商银行",
    price: 42.89,
    change: -0.37,
    market: "SH",
    sector: "银行",
  },
  {
    code: "000858",
    name: "五粮液",
    price: 126.3,
    change: 0.48,
    market: "SZ",
    sector: "白酒",
  },
];
export const signed = (n: number, digits = 2) =>
  `${n >= 0 ? "+" : ""}${n.toFixed(digits)}`;
export const direction = (n: number) => (n >= 0 ? "up" : "down");
export const previousClose = (s: Stock) => s.price / (1 + s.change / 100);
export type PanelKind = "chart" | "stocklist" | "depth" | "trades" | "metrics";
export const panelNames: Record<PanelKind, string> = {
  chart: "行情图表",
  stocklist: "股票列表",
  depth: "买卖十档",
  trades: "成交明细",
  metrics: "关键数据",
};
export type PreviewPanel = {
  id: string;
  kind: PanelKind;
  name: string;
  members: string[];
};
export type DockNode =
  | { type: "leaf"; panel: PreviewPanel }
  | { type: "split"; axis: "row" | "column"; children: DockNode[] };
export type DockSide = "left" | "right" | "top" | "bottom";
export type FloatingPanel = PreviewPanel & { x: number; y: number };
export type Board = {
  id: string;
  name: string;
  listName: string;
  members: string[];
  selected: string;
  mode: "intraday" | "daily";
  dock: DockNode;
  floating: FloatingPanel[];
};
export const newId = () =>
  globalThis.crypto?.randomUUID?.() ??
  `id-${Date.now()}-${Math.random().toString(36).slice(2)}`;
export const chartNode = (): DockNode => ({
  type: "leaf",
  panel: { id: newId(), kind: "chart", name: "行情图表", members: [] },
});
export function initialBoards(): Board[] {
  return [
    {
      id: "overview",
      name: "A 股全景",
      listName: "核心观察",
      members: stocks.map((s) => s.code),
      selected: stocks[0].code,
      mode: "intraday",
      dock: chartNode(),
      floating: [],
    },
    {
      id: "momentum",
      name: "短线观察",
      listName: "短线机会",
      members: ["603598", "001318", "601127", "300059", "002475"],
      selected: "001318",
      mode: "intraday",
      dock: chartNode(),
      floating: [],
    },
    {
      id: "trend",
      name: "趋势研究",
      listName: "趋势跟踪",
      members: ["300750", "688981", "002475", "000333"],
      selected: "300750",
      mode: "daily",
      dock: chartNode(),
      floating: [],
    },
  ];
}
export function removePanel(node: DockNode, id: string): DockNode | null {
  if (node.type === "leaf") return node.panel.id === id ? null : node;
  const children = node.children
    .map((n) => removePanel(n, id))
    .filter((n): n is DockNode => n !== null);
  return children.length === 0
    ? null
    : children.length === 1
      ? children[0]
      : { ...node, children };
}
export function splitDock(
  node: DockNode,
  panel: PreviewPanel,
  side: DockSide,
): DockNode {
  const added: DockNode = { type: "leaf", panel };
  return {
    type: "split",
    axis: side === "left" || side === "right" ? "row" : "column",
    children: side === "left" || side === "top" ? [added, node] : [node, added],
  };
}
export function parseImport(text: string, members: string[]) {
  const tokens = text.split(/[\s,，;；|\t]+/).filter(Boolean);
  const added: Stock[] = [];
  const existing: string[] = [];
  const unknown: string[] = [];
  const duplicates: string[] = [];
  const seen = new Set<string>();
  for (const token of tokens) {
    const code = token
      .toUpperCase()
      .replace(/^(SH|SZ)/, "")
      .replace(/\.(SH|SZ)$/, "");
    const stock = stocks.find((s) => s.code === code || s.name === token);
    if (!stock) {
      unknown.push(token);
      continue;
    }
    if (seen.has(stock.code)) {
      duplicates.push(token);
      continue;
    }
    seen.add(stock.code);
    if (members.includes(stock.code)) existing.push(stock.name);
    else added.push(stock);
  }
  return { added, existing, unknown, duplicates };
}
