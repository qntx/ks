/// Central class strings for the design system (design/desktop.mdx 视觉规范):
/// pill buttons, subtle-fill inputs, radius-24 dialogs on elevation 2,
/// segmented ToggleGroup, floating toasts. Components use these only —
/// no raw hex outside styles.css.
export const button = {
  primary:
    "inline-flex items-center justify-center gap-1.5 rounded-full bg-ink px-4 py-2 text-sm font-medium text-surface transition-opacity hover:opacity-85 disabled:opacity-40",
  secondary:
    "inline-flex items-center justify-center gap-1.5 rounded-full bg-subtle px-4 py-2 text-sm font-medium text-ink transition-colors hover:bg-line disabled:opacity-40",
  ghost:
    "inline-flex items-center justify-center gap-1.5 rounded-full px-3 py-1.5 text-sm text-muted transition-colors hover:bg-subtle hover:text-ink disabled:opacity-40",
  danger:
    "inline-flex items-center justify-center gap-1.5 rounded-full border border-danger/50 px-4 py-2 text-sm font-medium text-danger transition-colors hover:bg-danger/10 disabled:opacity-40",
};

export const input =
  "w-full rounded-2xl bg-subtle px-3 py-2 text-sm text-ink placeholder:text-muted focus:outline-none focus:ring-2 focus:ring-ink/30";

export const dialogOverlay =
  "fixed inset-0 z-40 bg-paper/70 backdrop-blur-sm data-[state=open]:animate-[fade-in_0.2s_ease-out]";

export const dialogContent =
  "fixed left-1/2 top-1/2 z-50 w-[min(92vw,26rem)] -translate-x-1/2 -translate-y-1/2 rounded-3xl bg-surface p-5 shadow-2 focus:outline-none data-[state=open]:animate-[pop-in_0.25s_var(--ease-fluid)]";

export const segmented = {
  root: "flex w-full rounded-full bg-subtle p-1",
  item: "flex-1 rounded-full px-3 py-1.5 text-sm font-medium whitespace-nowrap text-muted transition-colors data-[state=on]:bg-surface data-[state=on]:text-ink data-[state=on]:shadow-1",
};

export const switchTrack =
  "relative h-5 w-9 rounded-full bg-subtle transition-colors data-[state=checked]:bg-ink focus:outline-none";
export const switchThumb =
  "block h-4 w-4 translate-x-0.5 rounded-full bg-surface transition-transform data-[state=checked]:translate-x-[18px]";

export const tooltip = "z-50 rounded-full bg-ink px-3 py-1 text-xs text-surface";

export const toast =
  "flex items-center gap-2 rounded-full bg-ink px-4 py-2 text-sm text-surface shadow-2 data-[state=open]:animate-[rise_0.2s_var(--ease-fluid)]";

/// Layout surfaces.
export const panel = "flex w-70 shrink-0 flex-col rounded-3xl bg-surface shadow-1";
export const row =
  "flex w-full items-center gap-2 rounded-2xl px-3 py-1.5 text-left text-sm transition-colors";
export const rowSelected = "bg-subtle";
export const rowHover = "hover:bg-subtle/70";
export const card = "rounded-3xl bg-surface p-6 shadow-1";
export const fieldRow = "flex items-center gap-2 rounded-2xl bg-paper px-3 py-2";
export const title = "text-[17px] font-semibold";
export const displayTitle = "text-[28px] font-semibold tracking-[-0.025em]";
export const caption = "text-xs text-muted";
export const dragRegion = "h-7 shrink-0 w-full";
