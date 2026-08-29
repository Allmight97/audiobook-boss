import type { OutputNamingConfig } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';

export type OutputNamingPreset = OutputNamingConfig['preset'];

export type OutputPlanState = {
	readonly outputDirectory: string;
	readonly namingPreset: OutputNamingPreset;
	readonly namingTemplate: string;
	readonly previewTemplate: string;
	readonly absIncludeYear: boolean;
	readonly previewText: string;
	readonly previewTitle: string;
	readonly latestPreviewRequestId: number;
};

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
	readonly estimatedSizeText: string;
};

export type OutputPathPreviewMetadataDraft = AudiobookMetadata;

export const DEFAULT_CUSTOM_TEMPLATE = '{author}/{title}';
export const CUSTOM_TEMPLATE_PLACEHOLDER = '{author}/{series}/Book {seriesPart} - {title}';

export const EMPTY_PREVIEW_TEXT = 'Select output directory...';
export const EMPTY_PREVIEW_TITLE = 'No directory selected';

export function emptyOutputPlan(): OutputPlanState {
	return {
		outputDirectory: '',
		namingPreset: 'absDefault',
		namingTemplate: '',
		previewTemplate: '',
		absIncludeYear: false,
		previewText: EMPTY_PREVIEW_TEXT,
		previewTitle: EMPTY_PREVIEW_TITLE,
		latestPreviewRequestId: 0,
	};
}

export function namingHintText(preset: OutputNamingPreset, includeYear: boolean): string {
	if (preset !== 'absDefault') {
		return '';
	}
	return includeYear
		? 'Creates Author / Series / (Sub-series) / Book # - YYYY - Title'
		: 'Creates Author / Series / (Sub-series) / Book # - Title';
}

export function outputNamingFromPlan(plan: OutputPlanState): OutputNamingConfig {
	const trimmedTemplate = plan.previewTemplate.trim();
	return {
		preset: plan.namingPreset,
		includeYear: plan.absIncludeYear,
		customTemplate:
			plan.namingPreset === 'customTemplate'
				? trimmedTemplate.length > 0
					? plan.previewTemplate
					: DEFAULT_CUSTOM_TEMPLATE
				: undefined,
	};
}
