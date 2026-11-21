import { invoke } from '@tauri-apps/api/core';

export function rawInvoke(): typeof invoke {
	return invoke;
}
