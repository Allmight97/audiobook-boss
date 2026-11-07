import { describe, expect, it } from 'vitest';
import { audioFile } from '../../test/fixtures/fakeEngine';
import { cueNotice } from './cueNotice';
import { formatFileDetails, titleDetails } from './display';

const REMOVED =
	'Exported. Its download was removed to save space; acquire it again to export it again.';

describe('file details', () => {
	it('shows a failed file as an error', () => {
		const file = audioFile('/books/broken.m4b', {
			isValid: false,
			error: 'Broken audio source',
		});

		expect(formatFileDetails(file)).toBe('Error: Broken audio source');
	});

	it('names a failed file with no engine text as an invalid file', () => {
		expect(formatFileDetails(audioFile('/books/x.m4b', { isValid: false }))).toBe(
			'Error: Invalid file',
		);
	});

	it('shows an exported title whose download was removed without an error prefix', () => {
		const file = audioFile('/books/done.m4b', {
			isValid: false,
			sourceRemoved: true,
			error: REMOVED,
		});

		expect(formatFileDetails(file)).toBe(REMOVED);
	});

	it('shows a ready file by its facts, with --- for what the engine left out', () => {
		const file = audioFile('/books/a.m4b', {
			artist: 'Author',
			duration: 3600,
			size: 2048,
			format: 'M4B',
		});

		expect(formatFileDetails(file)).toBe('Author • 1:00:00 • 2.0 KB • M4B');
		expect(formatFileDetails({ ...file, duration: null, size: null })).toBe(
			'Author • --- • --- • M4B',
		);
	});
});

describe('title details', () => {
	const totals = {
		chapterCount: 3,
		durationSeconds: 7200,
		sizeBytes: 4096,
	};

	it('combines a ready group from the engine totals', () => {
		const anchor = audioFile('/books/a.m4b', { artist: 'Author', format: 'M4B' });

		expect(titleDetails(anchor, true, totals)).toBe('Author • 2:00:00 • 4.0 KB • M4B • 3 chapters');
		expect(titleDetails(anchor, true, undefined)).toBe('Author • --- • --- • M4B');
	});

	it('shows the engine text of a group that is not ready', () => {
		const failed = audioFile('/books/a.m4b', {
			status: 'failed',
			error: 'Broken audio source',
		});
		const removed = audioFile('/books/b.m4b', { status: 'removedFinished', error: REMOVED });

		expect(titleDetails(failed, true, totals)).toBe('Error: Broken audio source');
		expect(titleDetails(removed, true, totals)).toBe(REMOVED);
	});

	it('shows a single title by its own file', () => {
		const file = audioFile('/books/a.m4b', { duration: 60, size: 1024, format: 'M4B' });

		expect(titleDetails(file, false, totals)).toBe('1:00 • 1.0 KB • M4B');
	});
});

describe('cue notice', () => {
	const withCue = (
		status: 'ready' | 'needsConfirmation' | 'invalid' | 'ignored' | 'embeddedPreferred',
		actions: ('confirmHundredths' | 'ignore')[],
	) =>
		audioFile('/books/a.mp3', {
			chapterPlan: {
				chapters: [{ title: 'One', startMs: 0, endMs: 1000 }],
				fromCue: true,
				sourceFingerprint: '1:1',
			},
			cueSource: { fileName: 'a.cue', status, message: 'About the CUE.', actions },
		});

	it('offers exactly the actions the engine lists, labelled', () => {
		expect(cueNotice(withCue('needsConfirmation', ['confirmHundredths', 'ignore']))).toEqual({
			summary: '1 chapter from a.cue',
			message: 'About the CUE.',
			note: 'This CUE uses nonstandard timestamps. Interpret the final field as hundredths of a second?',
			actions: [
				{ choice: 'confirmHundredths', label: 'Use hundredths' },
				{ choice: 'ignore', label: 'Ignore CUE' },
			],
		});
		expect(cueNotice(withCue('ready', ['ignore']))?.note).toBe('CUE chapters accepted.');
		expect(cueNotice(withCue('ready', []))?.actions).toEqual([]);
	});

	it('names the chapter source only for a CUE that supplies chapters', () => {
		expect(cueNotice(withCue('embeddedPreferred', []))?.summary).toBe(
			'1 chapter from embedded audio',
		);
		expect(cueNotice(withCue('ignored', []))).toMatchObject({
			summary: null,
			note: 'CUE ignored. Converting without its chapters.',
		});
		expect(cueNotice(withCue('invalid', ['ignore']))?.summary).toBeNull();
	});

	it('has no notice without a CUE', () => {
		expect(cueNotice(audioFile('/books/a.m4b'))).toBeNull();
	});
});
