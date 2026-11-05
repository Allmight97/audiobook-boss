import type { FrontendAttachment } from '../generated/tauri';
import sessionFresh from '../generated/fixtures/session-fresh.json';
import settingsFresh from '../generated/fixtures/settings-fresh.json';
import { normalizeFrontendAttachment } from './normalizers';

const MACHINE_CONCURRENCY = 4;

const { concurrency } = settingsFresh;

export const recordedFresh = normalizeFrontendAttachment({
	client: 1,
	session: sessionFresh,
	settings: {
		...settingsFresh,
		concurrency: {
			...concurrency,
			effective: MACHINE_CONCURRENCY,
			capabilities: { ...concurrency.capabilities, autoEffective: MACHINE_CONCURRENCY },
		},
	},
} as FrontendAttachment);
