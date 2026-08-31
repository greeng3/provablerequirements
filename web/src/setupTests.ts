import "@testing-library/jest-dom";

// jsdom doesn't ship ResizeObserver, which @tanstack/react-virtual
// uses to track the scroll element's size. Stub it with a no-op so
// the virtualiser falls back to its initial estimate.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
if (typeof globalThis.ResizeObserver === "undefined") {
  (
    globalThis as unknown as { ResizeObserver: typeof ResizeObserverStub }
  ).ResizeObserver = ResizeObserverStub;
}

// jsdom's getBoundingClientRect returns zeros; give scroll
// containers a realistic viewport so react-virtual renders rows.
const originalGetBCR = Element.prototype.getBoundingClientRect;
Element.prototype.getBoundingClientRect = function getBCR(this: Element) {
  const base = originalGetBCR.call(this);
  if (base.width === 0 && base.height === 0) {
    return {
      ...base,
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      bottom: 600,
      right: 800,
      width: 800,
      height: 600,
      toJSON() {
        return this;
      },
    } as DOMRect;
  }
  return base;
};
