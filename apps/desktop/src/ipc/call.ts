import type { AppError } from "./bindings.gen";

export class IpcError extends Error {
  readonly code: string;

  constructor(error: AppError) {
    super(error.message);
    this.name = "IpcError";
    this.code = error.code;
  }
}

type IpcResult<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

/** Unwraps a typed command result, throwing `IpcError` on `error`. */
export async function call<T>(p: Promise<IpcResult<T>>): Promise<T> {
  const r = await p;
  if (r.status === "error") {
    throw new IpcError(r.error);
  }
  return r.data;
}
