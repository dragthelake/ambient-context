import { vi } from "vitest";

/// The Tauri boundary, replaced by a function per test. Every test names the
/// commands it expects; an unnamed command throws, so a component that grows
/// a new call fails loudly rather than silently getting undefined.
export type InvokeHandler = (
  command: string,
  args?: Record<string, unknown>,
) => unknown;

export type InvokeCall = { command: string; args?: Record<string, unknown> };

let handler: InvokeHandler = () => {
  throw new Error("no invoke handler installed");
};

export const calls: InvokeCall[] = [];
const listeners = new Map<string, (event: { payload: unknown }) => void>();

export const invoke = vi.fn(
  async (command: string, args?: Record<string, unknown>) => {
    calls.push({ command, args });
    return handler(command, args);
  },
);

export const listen = vi.fn(
  async (event: string, callback: (event: { payload: unknown }) => void) => {
    listeners.set(event, callback);
    return () => listeners.delete(event);
  },
);

/// Installs the handler and clears everything recorded so far.
export function mockInvoke(next: InvokeHandler): void {
  handler = next;
  calls.length = 0;
  listeners.clear();
  invoke.mockClear();
  listen.mockClear();
}

/// Fires a Tauri event at whatever subscribed through `listen`.
export function emit(event: string, payload: unknown): void {
  listeners.get(event)?.({ payload });
}

export function callsOf(command: string): InvokeCall[] {
  return calls.filter((call) => call.command === command);
}

export function countOf(command: string): number {
  return callsOf(command).length;
}
