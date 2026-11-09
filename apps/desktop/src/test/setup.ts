// happy-dom gaps used by Radix primitives and the item list.
export const happyDomStubs = true;

if (typeof Element.prototype.scrollIntoView !== "function") {
  Element.prototype.scrollIntoView = () => undefined;
}
if (typeof globalThis.ResizeObserver !== "function") {
  globalThis.ResizeObserver = class {
    observe(): void {
      /* not implemented in tests */
    }
    unobserve(): void {
      /* not implemented in tests */
    }
    disconnect(): void {
      /* not implemented in tests */
    }
  };
}
