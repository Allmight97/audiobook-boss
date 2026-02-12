import { describe, expect, it, beforeEach } from 'vitest';
import { calculateOutputPath } from '../outputPanel/pathBuilder';
import {
	updateOutputDirectory,
	updateAbsCompatible,
	updateAbsIncludeYear,
} from '../outputPanel/state';
import type { AudiobookMetadata } from '../../types/metadata';

const baseMetadata: AudiobookMetadata = {
	title: 'Ghosts',
	artist: 'Ryk Brown',
	series: 'Frontiers Saga',
	series_part: '7',
};

describe('calculateOutputPath', () => {
	beforeEach(() => {
		document.body.innerHTML = '<input id="merge-mode-toggle" type="checkbox" checked />';
		updateOutputDirectory('/Library/Audiobooks');
		updateAbsCompatible(true);
		updateAbsIncludeYear(false);
	});

	it('includes sub-series folder and prefers sub-series part', () => {
		const metadata: AudiobookMetadata = {
			...baseMetadata,
			subseries: 'Discovery',
			subseries_part: '1',
		};

		const path = calculateOutputPath(metadata);
		expect(path).toBe(
			'/Library/Audiobooks/Ryk Brown/Frontiers Saga/Part 1 - Discovery/Book 7 - Ghosts/Book 7 - Ghosts.m4b',
		);
	});

	it('keeps existing sub-series prefix', () => {
		const metadata: AudiobookMetadata = {
			...baseMetadata,
			subseries: 'Part 2 - Rogue Castes',
			subseries_part: '2',
		};

		const path = calculateOutputPath(metadata);
		expect(path).toBe(
			'/Library/Audiobooks/Ryk Brown/Frontiers Saga/Part 2 - Rogue Castes/Book 7 - Ghosts/Book 7 - Ghosts.m4b',
		);
	});
});
