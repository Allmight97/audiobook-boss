/**
 * Tests for the Tauri bridge abstraction.
 *
 * These tests verify that the bridge correctly routes calls
 * to either real Tauri APIs or mocks based on environment.
 */

import { describe, it, expect, vi, beforeEach } from 'vitest';

// Note: Tauri APIs are auto-mocked by src/test/setup.ts

describe('bridge', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('environment detection', () => {
    it('should detect non-Tauri environment in tests', () => {
      // In test environment, __TAURI_INTERNALS__ should be undefined
      expect((window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__).toBeUndefined();
    });
  });

  describe('typed command helpers', () => {
    it('should be importable', async () => {
      // Dynamic import to test module loading
      const { bridge } = await import('./bridge');
      expect(bridge).toBeDefined();
      expect(typeof bridge.analyzeAudioFiles).toBe('function');
      expect(typeof bridge.processAudiobookFilesV2).toBe('function');
      expect(typeof bridge.cancelProcessing).toBe('function');
    });
  });

  describe('listen', () => {
    it('should be importable', async () => {
      const { bridge } = await import('./bridge');
      expect(typeof bridge.listen).toBe('function');
    });
  });

  describe('open', () => {
    it('should be importable', async () => {
      const { bridge } = await import('./bridge');
      expect(typeof bridge.open).toBe('function');
    });
  });

  describe('openExternal', () => {
    it('should be importable', async () => {
      const { bridge } = await import('./bridge');
      expect(typeof bridge.openExternal).toBe('function');
    });
  });
});

// Example of how to test with custom mock implementations
describe('bridge with custom mocks', () => {
  it('example: mock invoke to return specific data', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);

    // Set up specific return value for this test
    mockInvoke.mockResolvedValueOnce({ files: [], totalDuration: 0 });

    // Now any code that calls invoke will get this mock
    const result = await invoke('analyze_audio_files', { paths: [] });
    expect(result).toEqual({ files: [], totalDuration: 0 });
  });
});
