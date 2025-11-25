/**
 * Vitest global setup file.
 *
 * This file runs before each test file and sets up:
 * - Tauri API mocks for testing outside the Tauri window
 * - DOM environment configuration
 * - Global test utilities
 */

import { vi } from 'vitest';

// Mock Tauri's invoke API
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string, _args?: unknown) => {
    console.warn(`[Test Mock] Unhandled invoke: ${cmd}`);
    return Promise.resolve(undefined);
  }),
}));

// Mock Tauri's event API
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockImplementation((event: string, _handler: unknown) => {
    console.warn(`[Test Mock] Unhandled listen: ${event}`);
    return Promise.resolve(() => {
      /* unlisten */
    });
  }),
  emit: vi.fn().mockResolvedValue(undefined),
}));

// Mock Tauri's dialog plugin
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn().mockResolvedValue(null),
  save: vi.fn().mockResolvedValue(null),
  message: vi.fn().mockResolvedValue(undefined),
  ask: vi.fn().mockResolvedValue(false),
  confirm: vi.fn().mockResolvedValue(false),
}));

// Mock Tauri's opener plugin
vi.mock('@tauri-apps/plugin-opener', () => ({
  openPath: vi.fn().mockResolvedValue(undefined),
  openUrl: vi.fn().mockResolvedValue(undefined),
}));

// Setup window.__TAURI_INTERNALS__ to false for tests
// This ensures bridge.ts uses the mock path
Object.defineProperty(window, '__TAURI_INTERNALS__', {
  value: undefined,
  writable: true,
});

// Set import.meta.env.DEV to true for tests
// This allows the bridge to use mocks in test environment
vi.stubEnv('DEV', true);
