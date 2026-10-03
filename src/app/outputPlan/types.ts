import type { OutputNamingConfig } from '../../types/audio';

export type OutputNamingPreset = OutputNamingConfig['preset'];

export type OutputView = {
	readonly outputDirectory: string;
	readonly namingPreset: OutputNamingPreset;
	readonly namingTemplate: string;
	readonly absIncludeYear: boolean;
	readonly previewText: string;
	readonly previewTitle: string;
	readonly absHintText: string;
	readonly absHintHidden: boolean;
	readonly templateRowHidden: boolean;
	readonly displayDirectory: string;
};

export const CUSTOM_TEMPLATE_PLACEHOLDER = '{author}/{series}/Book {seriesPart} - {title}';

export const EMPTY_PREVIEW_TEXT = 'Select output directory...';
export const EMPTY_PREVIEW_TITLE = 'No directory selected';
export const PREVIEW_UNAVAILABLE_TEXT =
	'Output preview unavailable. Fix metadata/template and retry.';

export function namingHintText(preset: OutputNamingPreset, includeYear: boolean): string {
	if (preset !== 'absDefault') {
		return '';
	}
	return includeYear
		? 'Creates Author / Series / (Sub-series) / Book # - YYYY - Title'
		: 'Creates Author / Series / (Sub-series) / Book # - Title';
}
