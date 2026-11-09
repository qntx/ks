/** Calls `fn` at most once per `ms`; trailing call fires after the globalThis. */
export function throttle<F extends (...args: never[]) => void>(
  fn: F,
  ms: number,
): (...args: Parameters<F>) => void {
  let last = 0;
  let pending: Parameters<F> | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  return (...args: Parameters<F>) => {
    const now = Date.now();
    const left = ms - (now - last);
    if (left <= 0) {
      last = now;
      fn(...args);
      return;
    }
    pending = args;
    timer ??= setTimeout(() => {
      timer = undefined;
      last = Date.now();
      if (pending) {
        fn(...pending);
      }
      pending = undefined;
    }, left);
  };
}
