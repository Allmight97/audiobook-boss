import { describe, expect, it } from 'vitest';
import {
	fileListNavigationCommandFromKey,
	resolveFileListNavigationTarget,
} from '../fileList/keyboardNavigation';

function keyEvent(
	key: string,
	modifiers: Partial<Pick<KeyboardEvent, 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>> = {},
): Pick<KeyboardEvent, 'altKey' | 'ctrlKey' | 'key' | 'metaKey' | 'shiftKey'> {
	return {
		key,
		altKey: modifiers.altKey ?? false,
		ctrlKey: modifiers.ctrlKey ?? false,
		metaKey: modifiers.metaKey ?? false,
		shiftKey: modifiers.shiftKey ?? false,
	};
}

describe('file list keyboard navigation', () => {
	it.each([
		['ArrowUp', 'previous'],
		['ArrowDown', 'next'],
		['Home', 'first'],
		['End', 'last'],
		['PageUp', 'pagePrevious'],
		['PageDown', 'pageNext'],
	] as const)('maps %s to %s', (key, command) => {
		expect(fileListNavigationCommandFromKey(keyEvent(key))).toBe(command);
	});

	it('ignores modified navigation keys', () => {
		expect(fileListNavigationCommandFromKey(keyEvent('ArrowDown', { shiftKey: true }))).toBeNull();
		expect(fileListNavigationCommandFromKey(keyEvent('End', { metaKey: true }))).toBeNull();
	});

	it('moves one item at a time and clamps at list edges', () => {
		expect(
			resolveFileListNavigationTarget({
				command: 'next',
				fileCount: 5,
				selectedIndex: 2,
			}),
		).toBe(3);
		expect(
			resolveFileListNavigationTarget({
				command: 'previous',
				fileCount: 5,
				selectedIndex: 0,
			}),
		).toBe(0);
	});

	it('jumps to first and last files', () => {
		expect(
			resolveFileListNavigationTarget({
				command: 'first',
				fileCount: 5,
				selectedIndex: 3,
			}),
		).toBe(0);
		expect(
			resolveFileListNavigationTarget({
				command: 'last',
				fileCount: 5,
				selectedIndex: 1,
			}),
		).toBe(4);
	});

	it('uses page jumps with a bounded page step', () => {
		expect(
			resolveFileListNavigationTarget({
				command: 'pageNext',
				fileCount: 25,
				selectedIndex: 4,
				pageStep: 10,
			}),
		).toBe(14);
		expect(
			resolveFileListNavigationTarget({
				command: 'pagePrevious',
				fileCount: 25,
				selectedIndex: 4,
				pageStep: 10,
			}),
		).toBe(0);
	});

	it('selects an edge when no file is selected', () => {
		expect(
			resolveFileListNavigationTarget({
				command: 'next',
				fileCount: 5,
				selectedIndex: -1,
			}),
		).toBe(0);
		expect(
			resolveFileListNavigationTarget({
				command: 'previous',
				fileCount: 5,
				selectedIndex: -1,
			}),
		).toBe(4);
	});
});
