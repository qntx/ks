export const en = {
  appTitle: "ks",
  vaultPath: "Vault",
  noVault: "No vault at the default path",
  git: "git",
  notGit: "not a git vault",
  agent: "agent",
  agentNone: "agent: not running",
  agentHost: (host: string): string => `agent: hosted by ${host}`,
  loading: "Reading vault status…",
  refresh: "Refresh",
};

export type Messages = typeof en;
