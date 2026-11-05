import { invoke } from '@tauri-apps/api/core';
import { describe, expect, it, vi } from 'vitest';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import recorded from '../generated/fixtures/settings-fresh.json';
import type { SettingsSnapshot } from '../../types/appSettings';
import { tauriClient } from './client';

function withoutMachineDependentFields({ concurrency, ...rest }: SettingsSnapshot) {
	const { effective: _effective, capabilities, ...preference } = concurrency;
	const { autoEffective: _autoEffective, ...fixedCapabilities } = capabilities;
	return { ...rest, concurrency: { ...preference, capabilities: fixedCapabilities } };
}

describe('recorded fresh-engine settings', () => {
	it('the fake engine starts with the settings a fresh engine reports', async () => {
		vi.mocked(invoke).mockResolvedValueOnce({
			outcome: { kind: 'applied' },
			snapshot: recorded,
		});

		const reply = await tauriClient.settingsDispatch(1, 0, {
			kind: 'setKeepAwake',
			enabled: true,
		});

		expect(withoutMachineDependentFields(createFakeEngine().settings())).toEqual(
			withoutMachineDependentFields(reply.snapshot),
		);
	});
});
