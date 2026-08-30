import type { InputView } from '../inputSession';
import type { MetadataView } from '../metadataSession';
import { METADATA_FIELD_DEFINITIONS } from '../metadataSession';
import type { OutputPathPreviewMetadataDraft } from './types';

export function sourcePathFromInput(input: InputView): string | undefined {
	const selected = [...input.selectedIndices].sort((left, right) => left - right);
	if (selected.length > 0) {
		return input.files[selected[0]]?.path;
	}
	return input.files.find((file) => file.isValid)?.path;
}

export function previewDraftFromMetadataView(view: MetadataView): OutputPathPreviewMetadataDraft {
	const draft: OutputPathPreviewMetadataDraft = {
		title: '',
		album: '',
		artist: '',
		composer: '',
		genre: '',
		description: '',
		series: '',
		subseries: '',
	};

	for (const field of METADATA_FIELD_DEFINITIONS) {
		const raw = view.form.fields[field.inputId].value.trim();
		if (field.key === 'date') {
			if (raw) {
				draft.date = raw;
			}
			continue;
		}
		(draft as Record<string, unknown>)[field.key] = raw;
		if ('mapToAlbum' in field && field.mapToAlbum && field.key === 'title') {
			draft.album = raw || draft.title;
		}
	}

	if (!draft.album) {
		draft.album = draft.title ?? '';
	}

	const cover = view.cover.currentCoverArt;
	if (cover && cover.length > 0 && !view.cover.coverArtRemovalRequested) {
		draft.cover_art = cover;
	}

	return draft;
}
