import type { FrontendAttachment } from '../generated/tauri';
import sessionFresh from '../generated/fixtures/session-fresh.json';
import settingsFresh from '../generated/fixtures/settings-fresh.json';
import { normalizeFrontendAttachment } from './normalizers';

export const recordedFresh = normalizeFrontendAttachment({
	client: 1,
	session: sessionFresh,
	settings: settingsFresh,
} as FrontendAttachment);
