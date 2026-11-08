import type { OutputNamingConfig } from '../../lib/generated/tauri';

type OutputNamingPreset = OutputNamingConfig['preset'];

export type OutputView = {
	readonly outputDirectory: string;
	readonly namingPreset: OutputNamingPreset;
	readonly namingTemplate: string;
	readonly absIncludeYear: boolean;
	readonly previewText: string;
	readonly previewTitle: string;
	readonly templateRowHidden: boolean;
	readonly displayDirectory: string;
};

export const CUSTOM_TEMPLATE_PLACEHOLDER = '{author}/{series}/Book {seriesPart} - {title}';

export const EMPTY_PREVIEW_TEXT = 'Select output directory...';
export const EMPTY_PREVIEW_TITLE = 'No directory selected';
