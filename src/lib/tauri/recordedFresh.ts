import type { FrontendAttachment } from '../generated/tauri';
import sessionFresh from '../generated/fixtures/session-fresh.json';
import settingsFresh from '../generated/fixtures/settings-fresh.json';
import { normalizeFrontendAttachment } from './normalizers';

/* oxlint-disable typescript/no-unsafe-type-assertion -- a JSON import widens the generated unions to string; the exporter writes these files from the generated types. */
export const recordedFresh = normalizeFrontendAttachment({
	client: 1,
	session: sessionFresh,
	settings: settingsFresh,
} as FrontendAttachment);
/* oxlint-enable typescript/no-unsafe-type-assertion */
