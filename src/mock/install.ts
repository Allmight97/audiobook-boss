import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';

import { handleInvoke } from './runtime';

export function installMockRuntime(): void {
	mockWindows('main');
	mockIPC((cmd, payload) => handleInvoke(cmd, payload), { shouldMockEvents: true });
}

installMockRuntime();
