// jsdom has no ResizeObserver. DefragMap measures its own width to decide
// how many columns to draw, so every test that renders it needs a stub.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}

globalThis.ResizeObserver = ResizeObserverStub as unknown as typeof ResizeObserver;
