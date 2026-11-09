export type ToastItem = { id: number; text: string; tone: "success" | "error" };

let items: ToastItem[] = [];
let nextId = 0;
const listeners = new Set<() => void>();

function emit(): void {
  for (const l of listeners) {
    l();
  }
}

export function subscribeToasts(l: () => void): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

export function getToasts(): ToastItem[] {
  return items;
}

export function dismissToast(id: number): void {
  items = items.filter((t) => t.id !== id);
  emit();
}

/** Queues a bottom-center toast pill for 3s. */
export function toast(text: string, opts?: { tone?: "success" | "error" | undefined }): void {
  const id = ++nextId;
  items = [...items, { id, text, tone: opts?.tone ?? "success" }];
  emit();
}
