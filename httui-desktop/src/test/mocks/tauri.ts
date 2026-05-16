import { vi } from "vitest";

const handlers: Record<string, (...args: unknown[]) => unknown> = {};

export function mockTauriCommand(
  cmd: string,
  handler: (...args: unknown[]) => unknown,
) {
  handlers[cmd] = handler;
}

export function clearTauriMocks() {
  for (const key of Object.keys(handlers)) {
    delete handlers[key];
  }
}

export const invoke = vi.fn(
  async (cmd: string, args?: Record<string, unknown>) => {
    const handler = handlers[cmd];
    if (handler) return handler(args);
    return undefined;
  },
);

/**
 * Test double for `@tauri-apps/api/core`'s `Channel`. The real one is a
 * serializable transport; here it's just an `onmessage` sink. A mocked
 * `invoke` handler receives the channel as `args.onChunk` and can push
 * chunks by calling `chunk.onmessage?.(msg)` to simulate the backend.
 */
export class Channel<T = unknown> {
  onmessage: ((msg: T) => void) | null = null;
}
