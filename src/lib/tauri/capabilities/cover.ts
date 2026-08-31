import { tauriClient } from '../client';

export type CoverArtView = {
	readonly handleId: string | null;
	readonly dataUrl: string;
};

export function stagedCoverHandle(view: CoverArtView): string | null {
	return view.handleId && view.handleId.length > 0 ? view.handleId : null;
}

export interface CoverCapability {
	thumbnail(path: string): Promise<CoverArtView | null>;
	previewFromUrl(url: string): Promise<CoverArtView>;
	stageFromFile(filePath: string): Promise<CoverArtView>;
	stageFromUrl(url: string): Promise<CoverArtView>;
}

function normalizeCoverArtView(view: {
	readonly handleId?: string | null;
	readonly dataUrl: string;
}): CoverArtView {
	return {
		handleId: view.handleId ?? null,
		dataUrl: view.dataUrl,
	};
}

export const liveCoverCapability: CoverCapability = {
	async thumbnail(path) {
		const view = await tauriClient.readAudioCoverThumbnail(path);
		return view ? normalizeCoverArtView(view) : null;
	},
	async previewFromUrl(url) {
		return normalizeCoverArtView(await tauriClient.previewCoverArtFromUrl(url));
	},
	async stageFromFile(filePath) {
		return normalizeCoverArtView(await tauriClient.loadCoverArtFile(filePath));
	},
	async stageFromUrl(url) {
		return normalizeCoverArtView(await tauriClient.loadCoverArtFromUrl(url));
	},
};
