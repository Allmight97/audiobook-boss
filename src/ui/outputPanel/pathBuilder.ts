/**
 * Output path building and sanitization utilities
 */
import type { AudiobookMetadata } from '../../types/metadata';
import { getJobType } from '../jobControls';
import {
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
} from '../metadataValidation';
import { getOutputNamingConfig, getState } from './state';

/**
 * Sanitizes a string for use in filenames by replacing problematic characters
 */
export function sanitizeFilename(
	input: string,
	options: { preserveCommas?: boolean } = {},
): string {
	const preserveCommas = options.preserveCommas ?? false;
	let value = input.replace(/:/g, ' - ');
	if (!preserveCommas) {
		value = value.replace(/,/g, ' - ');
	}
	return value
		.replace(/[/\\*?"<>|]/g, ' ')
		.split(/\s+/)
		.filter(Boolean)
		.join(' ');
}

function isSubseriesPrefixed(value: string): boolean {
	const trimmed = value.trimStart().toLowerCase();
	return ['part', 'book', 'vol', 'vol.', 'volume'].some((prefix) => trimmed.startsWith(prefix));
}

function normalizeSubseriesLabel(subseries: string, subseriesPart: string | undefined): string {
	if (subseriesPart && !isSubseriesPrefixed(subseries)) {
		return `Part ${subseriesPart} - ${subseries}`;
	}
	return subseries;
}

function buildAbsTitle(
	title: string,
	seriesPart: string | undefined,
	year: number | string | undefined,
	includeYear: boolean,
): string {
	if (includeYear && year !== undefined) {
		if (seriesPart) {
			return `Book ${seriesPart} - ${String(year)} - ${title}`;
		}
		return `${String(year)} - ${title}`;
	}

	if (seriesPart) {
		return `Book ${seriesPart} - ${title}`;
	}
	return title;
}

function buildSimpleFilename(
	title: string,
	year: number | string | undefined,
	includeYear: boolean,
): string {
	let base = title;
	if (includeYear && year !== undefined) {
		base = `${base} (${year})`;
	}
	return `${base}.m4b`;
}

/**
 * Calculates the full output path based on current settings for PREVIEW
 */
export function calculateOutputPath(metadata: AudiobookMetadata): string {
	const state = getState();
	const naming = getOutputNamingConfig();
	let basePath = state.outputDirectory || '[Output Directory]';
	const jobType = getJobType();

	if (jobType === 'batch') {
		basePath += '/(Batch Output Folder)';
		const placeholderAuthor = '[Author]';
		const placeholderSeries = '[Series]';
		const placeholderSubseries = '[Sub-series]';
		const placeholderTitle = '[Title]';
		const placeholderSeriesPart = '#';
		const placeholderYear = 'YYYY';

		if (naming.absCompatible) {
			const subseriesLabel = normalizeSubseriesLabel(placeholderSubseries, placeholderSeriesPart);
			const absTitle = buildAbsTitle(
				placeholderTitle,
				placeholderSeriesPart,
				naming.includeYear ? placeholderYear : undefined,
				naming.includeYear,
			);
			return `${basePath}/${placeholderAuthor}/${placeholderSeries}/${subseriesLabel}/${absTitle}/${absTitle}.m4b`;
		}

		const filename = buildSimpleFilename(
			placeholderTitle,
			naming.includeYear ? placeholderYear : undefined,
			naming.includeYear,
		);
		return `${basePath}/${filename}`;
	}

	const author =
		sanitizeFilename(metadata.artist || 'Unknown Author', {
			preserveCommas: true,
		}) || 'Unknown Author';
	const title = sanitizeFilename(metadata.title || 'Untitled') || 'Untitled';
	const series = sanitizeFilename(metadata.series || '');
	const subseries = sanitizeFilename(metadata.subseries || '');
	const year = typeof metadata.date === 'number' ? metadata.date : undefined;

	if (naming.absCompatible) {
		let subdirPath = `${basePath}/${author}`;
		if (series) {
			subdirPath += `/${series}`;
		}
		if (subseries) {
			const rawSubseriesPart = metadata.subseries_part || '';
			const subseriesPartError = getSubseriesPartValidationError(rawSubseriesPart);
			const subseriesPartValue = subseriesPartError
				? ''
				: sanitizeFilename(rawSubseriesPart).trim();
			const subseriesPart = subseriesPartValue.length > 0 ? subseriesPartValue : undefined;
			const subseriesLabel = normalizeSubseriesLabel(subseries, subseriesPart);
			subdirPath += `/${subseriesLabel}`;
			const rawSeriesPart = metadata.series_part || '';
			const seriesPartError = getSeriesPartValidationError(rawSeriesPart);
			const seriesPartValue = seriesPartError ? '' : sanitizeFilename(rawSeriesPart).trim();
			const seriesPart = seriesPartValue.length > 0 ? seriesPartValue : undefined;
			const bookPart = seriesPart;
			const absTitle = buildAbsTitle(title, bookPart, year, naming.includeYear);
			return `${subdirPath}/${absTitle}/${absTitle}.m4b`;
		}

		const rawSeriesPart = metadata.series_part || '';
		const seriesPartError = getSeriesPartValidationError(rawSeriesPart);
		const seriesPartValue = seriesPartError ? '' : sanitizeFilename(rawSeriesPart).trim();
		const seriesPart = seriesPartValue.length > 0 ? seriesPartValue : undefined;
		const absTitle = buildAbsTitle(title, seriesPart, year, naming.includeYear);
		return `${subdirPath}/${absTitle}/${absTitle}.m4b`;
	}

	const filename = buildSimpleFilename(
		title,
		naming.includeYear ? year : undefined,
		naming.includeYear,
	);
	return `${basePath}/${filename}`;
}
