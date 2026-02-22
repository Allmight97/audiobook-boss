import { describe, expect, it, beforeEach } from 'vitest';
import { calculateOutputPath } from '../outputPanel/pathBuilder';
import { setJobTypeSelection } from '../jobControls';
import {
	updateOutputDirectory,
	updateAbsIncludeYear,
	updateNamingPreset,
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
		updateOutputDirectory('/Library/Audiobooks');
		updateNamingPreset('absDefault');
		updateAbsIncludeYear(false);
		setJobTypeSelection('merge');
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
