import { vi, type Mock } from 'vitest';

export const invokeMock: Mock<(command: string, args?: unknown) => Promise<unknown>> = vi.fn();
