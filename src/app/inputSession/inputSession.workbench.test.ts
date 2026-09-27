import { afterEach, describe, expect, it } from 'vitest';
import type { AudioFile } from '../../types/audio';
import {
	clearAllFilesFromSession,
	moveFileInSession,
	reorderFilesInSession,
	restoreImportOrderInSession,
	sortFilesInSession,
} from './order';
import { clearSelectionInSession, selectAllInSession, selectFileInSession } from './selection';
import { emptyInputSession, type InputSessionState } from './types';
import { toInspectorView } from './inspector';

function file(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
	return {
		path,
		inputId: path,
		isValid: true,
		duration: 60,
		size: 1024,
		format: 'm4b',
		...overrides,
	};
}

function sessionWith(files: AudioFile[], selected: number[] = []): InputSessionState {
	const importOrdinalByPath: Record<string, number> = {};
	files.forEach((entry, index) => {
		importOrdinalByPath[entry.path] = index;
	});
	return {
		...emptyInputSession(),
		files,
		selectedIndices: selected,
		selectedAnchor: selected[selected.length - 1] ?? -1,
		importOrdinalByPath,
		nextImportOrdinal: files.length,
	};
}

describe('input session workbench mutations', () => {
	afterEach(() => undefined);

	it('selects, ranges, and clears without mutating files', () => {
		const session = sessionWith([file('/a'), file('/b'), file('/c')]);
		const selected = selectFileInSession(session, 1, { multi: false, range: false });
		const ranged = selectFileInSession(selected, 2, { multi: false, range: true });
		expect(ranged.selectedIndices).toEqual([1, 2]);
		expect(selectAllInSession(ranged).selectedIndices).toEqual([0, 1, 2]);
		expect(clearSelectionInSession(ranged).selectedIndices).toEqual([]);
		expect(ranged.files).toEqual(session.files);
	});

	it('reorders and sorts while preserving selected identity', () => {
		const session = selectFileInSession(
			sessionWith([file('/books/c.m4b'), file('/books/a.m4b'), file('/books/b.m4b')]),
			0,
			{ multi: false, range: false },
		);
		const moved = moveFileInSession(session, 0, 'down');
		expect(moved.files.map((entry) => entry.path)).toEqual([
			'/books/a.m4b',
			'/books/c.m4b',
			'/books/b.m4b',
		]);
		expect(moved.selectedIndices).toEqual([1]);
		const sorted = sortFilesInSession(moved);
		expect(sorted.files.map((entry) => entry.path)).toEqual([
			'/books/a.m4b',
			'/books/b.m4b',
			'/books/c.m4b',
		]);
		expect(sorted.files[sorted.selectedAnchor]?.path).toBe('/books/c.m4b');
		const restored = restoreImportOrderInSession(sorted);
		expect(restored.files.map((entry) => entry.path)).toEqual([
			'/books/c.m4b',
			'/books/a.m4b',
			'/books/b.m4b',
		]);
	});

	it('reorders by pointer insert index', () => {
		const session = sessionWith([file('/a'), file('/b'), file('/c'), file('/d'), file('/e')]);
		expect(reorderFilesInSession(session, 0, 2).files.map((entry) => entry.path)).toEqual([
			'/b',
			'/c',
			'/a',
			'/d',
			'/e',
		]);
	});

	it('blocks mutations while order is locked', () => {
		const session = { ...sessionWith([file('/a'), file('/b')]), orderLocked: true };
		expect(moveFileInSession(session, 0, 'down')).toBe(session);
		expect(clearAllFilesFromSession(session)).toBe(session);
	});

	it('projects inspector values for single and mixed selections', () => {
		const session = selectFileInSession(
			sessionWith([
				file('/a', { bitrate: 64_000, sampleRate: 44100, channels: 2, codecLabel: 'AAC' }),
				file('/b', { bitrate: 96_000, sampleRate: 48000, channels: 1, codecLabel: 'AAC' }),
			]),
			0,
			{ multi: false, range: false },
		);
		const single = toInspectorView(session, () => ({ text: '---', title: '' }));
		expect(single.contextVariant).toBe('single');
		expect(single.bitrateText).toBe('64 kbps');
		const multi = toInspectorView(selectAllInSession(session), () => ({
			text: '1 PDF',
			title: 'notes.pdf',
		}));
		expect(multi.contextText).toBe('2 files selected');
		expect(multi.codecText).toBe('AAC');
		expect(multi.companionsText).toBe('1 PDF');
	});
});
