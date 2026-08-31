import { vi } from 'vitest';
import type { CoverArtView, CoverCapability } from '../../lib/tauri/capabilities/cover';

export const JPEG_DATA_URL = 'data:image/jpeg;base64,/9j/4AAQ';

export function previewCoverView(dataUrl = JPEG_DATA_URL): CoverArtView {
	return { handleId: null, dataUrl };
}

export function stagedCoverView(handleId = 'cover-1', dataUrl = JPEG_DATA_URL): CoverArtView {
	return { handleId, dataUrl };
}

export function fakeCoverCapability(overrides: Partial<CoverCapability> = {}): CoverCapability {
	return {
		thumbnail: vi.fn(async () => null),
		previewFromUrl: vi.fn(async () => previewCoverView()),
		stageFromFile: vi.fn(async () => stagedCoverView()),
		stageFromUrl: vi.fn(async () => stagedCoverView()),
		...overrides,
	};
}
