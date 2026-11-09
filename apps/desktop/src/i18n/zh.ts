import type { Messages } from "./en";

export const zh: Messages = {
  appTitle: "ks",
  vaultPath: "保险库",
  noVault: "默认路径下没有保险库",
  git: "git",
  notGit: "非 git 保险库",
  agent: "agent",
  agentNone: "agent：未运行",
  agentHost: (host: string): string => `agent：由 ${host} 托管`,
  loading: "正在读取保险库状态…",
  refresh: "刷新",
};
