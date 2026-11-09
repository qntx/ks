// Dev-only in-browser vault so the UI runs outside Tauri (`?mock`,
// `?mock=locked`, `?mock=unlocked`). Never imported by production code.
import { emit } from "@tauri-apps/api/event";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const PASS = "correct horse battery staple";
const VAULT = "/Users/demo/.ks";

const settings = {
  autoLockMinutes: 15,
  maxSessionHours: 8,
  clipboardSeconds: 45,
  lockOnSleep: true,
  language: "system",
  syncOnUnlock: true,
  syncOnLock: true,
  syncAfterWrite: false,
};

const texts = new Map<string, string>([
  [
    "work/github",
    "demo-github-token-not-real\nuser: octocat\nurl: https://github.com\ntotp: GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ\nnotes: Main account",
  ],
  ["work/aws/root", "demo-aws-access-key\nuser: root\nnotes: Break glass only"],
  ["work/aws/staging", "staging-secret-key\nuser: deploy\nurl: https://console.aws.amazon.com"],
  ["work/stripe", "demo-stripe-key-not-real\nurl: https://stripe.com"],
  ["work/linear", "demo-linear-key\nuser: me@example.com"],
  ["personal/bank", "r0sebud-1996\nuser: 4471 2200\nnotes: OTP arrives by SMS"],
  [
    "personal/email",
    "correct horse battery staple\nuser: me@example.com\nurl: https://mail.example.com",
  ],
  ["personal/ssh/laptop", "demo-ssh-private-key"],
  ["personal/wifi", "hunter2\nnotes: Home network, 5GHz"],
  ["finance/wise", "wise-pass-9\nuser: xu@example.com"],
  ["finance/paypal", "paypal-pass\nuser: xu@example.com"],
]);
const binaries = new Map<string, number>([["personal/passport.pdf", 248_102]]);

let hasVault = false;
let unlocked = false;
let lastSyncState: unknown = { status: "idle" };
let syncUsed = false;

const params = new URLSearchParams(location.search);
const DOCTOR_PROBLEMS = [
  {
    path: `${VAULT}/.identity.age`,
    kind: "unfinished_rotation",
    message: "unfinished key rotation; run `ks rekey` with the new passphrase to finish",
    fixable: false,
  },
  {
    path: `${VAULT}/.tmpabc`,
    kind: "temp_file",
    message: "leftover temp file",
    fixable: true,
  },
  {
    path: `${VAULT}/.gitignore`,
    kind: "git_meta",
    message: ".gitignore differs from the canonical content",
    fixable: true,
  },
  {
    path: `${VAULT}/work/github.age`,
    kind: "undecryptable",
    message: "decryption failed (possibly written before a key rotation)",
    fixable: false,
  },
];
const conflictParam = params.get("conflict");
const conflictKind =
  conflictParam === "content" || conflictParam === "delete" ? conflictParam : null;
const mode = params.get("mock");
let touchIdEnabled = params.get("touchid") !== null;
let keychainEnabled = params.get("keychain") !== null;
const deviceStale = params.get("stale") !== null;
if (mode === "locked" || mode === "unlocked") {
  hasVault = true;
  unlocked = mode === "unlocked";
}

const fields = (text: string): Array<[string, string]> =>
  text
    .split("\n")
    .slice(1)
    .flatMap((line) => {
      const i = line.indexOf(": ");
      const key = i > 0 ? line.slice(0, i) : "";
      return i > 0 && !/[\s]/.test(key) ? [[key, line.slice(i + 2)]] : [];
    });

type ErrObj = { status: "error"; error: { code: string; message: string } };
const errObj = (code: string, message: string): ErrObj => ({
  status: "error",
  error: { code, message },
});
const needUnlock = (): ErrObj | undefined =>
  unlocked ? undefined : errObj("locked", "the vault is locked");

const names = (): string[] => [...texts.keys(), ...binaries.keys()].toSorted();

const detail = (name: string): Record<string, unknown> => {
  const text = texts.get(name);
  if (text === undefined) {
    if (binaries.has(name)) {
      return {
        name,
        kind: "binary",
        value: null,
        fields: [],
        notesLength: 0,
        hasTotp: false,
        binaryLength: binaries.get(name),
      };
    }
    return errObj("not_found", `no entry named ${name}`);
  }
  const fs = fields(text).filter(([k]) => k !== "notes" && k !== "totp");
  return {
    name,
    kind: "text",
    value: { key: "value", length: (text.split("\n")[0] ?? "").length },
    fields: fs.map(([key, v]) => ({ key, length: v.length })),
    notesLength: fields(text).find(([k]) => k === "notes")?.[1].length ?? 0,
    hasTotp: fields(text).some(([k]) => k === "totp"),
    binaryLength: null,
  };
};

const resolveField = (name: string, field: unknown): string | ErrObj => {
  const text = texts.get(name);
  if (text === undefined) {
    return errObj("not_found", `no entry named ${name}`);
  }
  if (field === "value") {
    return text.split("\n")[0] ?? "";
  }
  if (field === "totp") {
    return String(((Math.floor(Date.now() / 1000) * 7) % 900000) + 100000);
  }
  const key =
    field === "notes"
      ? "notes"
      : typeof field === "object" && field !== null && "field" in field
        ? String(field.field)
        : "";
  const hit = fields(text).find(([k]) => k === key);
  if (hit === undefined) {
    return errObj("not_found", key);
  }
  return hit[1];
};

const validFor = (): number => 30 - (Math.floor(Date.now() / 1000) % 30);

const isRecord = (v: unknown): v is Record<string, unknown> =>
  v !== null &&
  typeof v === "object" &&
  !Array.isArray(v) &&
  !(v instanceof ArrayBuffer) &&
  !ArrayBuffer.isView(v);

const changed = (): void => {
  void emit("entries-changed", null);
};

mockWindows("main");
/** Real invokes reject on error — `typedError` maps rejections to `{status:"error"}`. */
const dispatch = (cmd: string, payload: unknown): unknown => {
  const args: Record<string, unknown> = isRecord(payload) ? payload : {};
  const arg = (k: string): string => {
    const v = args[k];
    return typeof v === "string"
      ? v
      : typeof v === "number" || typeof v === "boolean"
        ? String(v)
        : "";
  };
  const gate = needUnlock();
  switch (cmd) {
    case "app_state":
      return {
        vault: hasVault ? { path: VAULT, isGit: true, warnings: [] } : null,
        unlocked,
        lockReason: null,
        settings,
      };
    case "recent_vaults":
      return [VAULT];
    case "vault_status":
      return { path: VAULT, exists: hasVault, isGit: true, agent: null };
    case "open_vault":
      hasVault = true;
      unlocked = false;
      return { path: arg("path"), isGit: true, warnings: [] };
    case "create_vault":
    case "clone_vault":
      hasVault = true;
      unlocked = true;
      return { path: arg("path") === "" ? VAULT : arg("path"), isGit: true, warnings: [] };
    case "passphrase_strength": {
      const n = arg("passphrase").length;
      const score = n < 8 ? 1 : n < 12 ? 2 : n < 20 ? 3 : 4;
      return { score, ok: n >= 8, problem: n < 8 ? "Too short" : null, suggestions: [] };
    }
    case "unlock":
      if (arg("passphrase") !== PASS) {
        return errObj("wrong_passphrase", "Wrong passphrase");
      }
      unlocked = true;
      return { warnings: [] };
    case "git_status":
      return {
        branch: "main",
        upstream: "origin/main",
        ahead: 1,
        behind: 0,
        changes: params.get("clean") === null ? 2 : 0,
      };
    case "sync_state":
      return lastSyncState;
    case "doctor":
      if (params.get("doctor") === "error") {
        return errObj("unavailable", "cannot open the vault index");
      }
      return { problems: params.get("doctor") === null ? [] : DOCTOR_PROBLEMS };
    case "doctor_fix":
      return { problems: DOCTOR_PROBLEMS.filter((p) => !p.fixable) };
    case "change_passphrase":
      return [];
    case "rotate_key":
      return { entries: 3, resumed: params.get("rotation") !== null, warnings: [] };
    case "rotation_pending":
      return params.get("rotation") !== null;
    case "sync_now":
      if (!syncUsed && conflictKind !== null) {
        syncUsed = true;
        lastSyncState = {
          status: "conflict",
          entries: ["mail/github", "bank"],
          kind: conflictKind,
        };
        void emit("sync-state", lastSyncState);
        return lastSyncState;
      }
      lastSyncState = { status: "running" };
      void emit("sync-state", lastSyncState);
      setTimeout(() => {
        lastSyncState = {
          status: "synced",
          at_secs: Math.floor(Date.now() / 1000),
        };
        void emit("sync-state", lastSyncState);
        void emit("entries-changed", null);
      }, 600);
      return { status: "synced", committed: false, pulled: true, pushed: true, warnings: [] };

    case "devices":
      return {
        touchId: { supported: true, enabled: touchIdEnabled, reason: null },
        keychain: { supported: true, enabled: keychainEnabled, reason: null },
      };
    case "set_device":
      if (arg("kind") === "touch_id") {
        touchIdEnabled = Boolean(args["enabled"]);
      }
      if (arg("kind") === "keychain") {
        keychainEnabled = Boolean(args["enabled"]);
      }
      return [];

    case "unlock_device":
      if (deviceStale) {
        return { unlocked: false, warnings: [], stale: true };
      }
      if (touchIdEnabled || keychainEnabled) {
        unlocked = true;
        return { unlocked: true, warnings: [], stale: false };
      }
      return { unlocked: false, warnings: [], stale: false };
    case "lock":
      unlocked = false;
      void emit("locked", { reason: "manual" });
      return null;
    case "heartbeat":
      return null;
    case "list_entries":
      return gate ?? names();
    case "entry_detail":
      return gate ?? detail(arg("name"));
    case "reveal_field":
      return gate ?? resolveField(arg("name"), args["field"]);
    case "copy_field": {
      const miss = gate ?? resolveField(arg("name"), args["field"]);
      return typeof miss === "object" ? miss : { clearsInSecs: settings.clipboardSeconds };
    }
    case "totp_code":
      return (
        gate ?? {
          code: String(((Math.floor(Date.now() / 1000) * 7) % 900000) + 100000),
          validFor: validFor(),
        }
      );
    case "edit_entry":
      return gate ?? texts.get(arg("name")) ?? "";
    case "save_entry":
      if (gate !== undefined) {
        return gate;
      }
      texts.set(arg("name"), arg("text"));
      changed();
      return [];
    case "rename_entry":
      if (gate !== undefined) {
        return gate;
      }
      texts.set(arg("to"), texts.get(arg("from")) ?? "");
      texts.delete(arg("from"));
      changed();
      return [];
    case "delete_entry":
      if (gate !== undefined) {
        return gate;
      }
      texts.delete(arg("name"));
      binaries.delete(arg("name"));
      changed();
      return [];
    case "import_binary":
      if (gate !== undefined) {
        return gate;
      }
      binaries.set(arg("name"), 128_000);
      changed();
      return [];
    case "export_binary":
      return gate ?? null;
    case "generate_password": {
      const sets = {
        alphanum: "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789",
        printable:
          "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+[]{};:,.<>/?",
        hex: "0123456789abcdef",
        slug: "abcdefghijklmnopqrstuvwxyz0123456789",
      } as const;
      const cs = arg("charset");
      const set =
        cs === "alphanum" || cs === "printable" || cs === "hex" || cs === "slug"
          ? sets[cs]
          : sets.alphanum;
      let out = "";
      for (let i = 0; i < Number(arg("length")); i++) {
        out += set.charAt(i % set.length);
      }
      return out;
    }
    case "get_settings":
      return settings;
    case "set_settings":
      Object.assign(settings, args["s"]);
      return null;
    case "plugin:dialog|open":
      return "/Users/demo/Documents/passport.pdf";
    case "plugin:dialog|save":
      return "/Users/demo/Downloads/export.bin";
    default:
      return null;
  }
};

mockIPC(
  (cmd, payload) => {
    const out = dispatch(cmd, payload);
    if (isRecord(out) && out["status"] === "error") {
      throw out["error"];
    }
    return out;
  },
  { shouldMockEvents: true },
);
