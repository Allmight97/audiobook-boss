import { describe, expect, it, vi } from 'vitest';
import {
	fileListNavigationCommandFromKey,
	interpretFileListKeyDown,
	resolveFileListNavigationTarget,
} from '.';

function keyEvent(
	key: string,
	options: Partial<Pick<KeyboardEvent, 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>> & {
		target?: EventTarget;
	} = {},
): KeyboardEvent {
	return {
		key,
		altKey: options.altKey ?? false,
		ctrlKey: options.ctrlKey ?? false,
		metaKey: options.metaKey ?? false,
		shiftKey: options.shiftKey ?? false,
		target: options.target ?? document.createElement('div'),
		preventDefault: vi.fn(),
	} as unknown as KeyboardEvent;
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

	it('moves one file at a time and stays put at a list edge', () => {
		const list = { fileCount: 5 };
		expect(resolveFileListNavigationTarget({ ...list, command: 'next', selectedIndex: 2 })).toBe(3);
		expect(
			resolveFileListNavigationTarget({ ...list, command: 'previous', selectedIndex: 0 }),
		).toBe(0);
		expect(interpretFileListKeyDown(keyEvent('ArrowDown'), { ...list, selectedAnchor: 4 })).toEqual(
			{ type: 'navigate', index: 4 },
		);
	});

	it('jumps by a bounded page step', () => {
		const list = { fileCount: 25, selectedIndex: 4, pageStep: 10 };
		expect(resolveFileListNavigationTarget({ ...list, command: 'pageNext' })).toBe(14);
		expect(resolveFileListNavigationTarget({ ...list, command: 'pagePrevious' })).toBe(0);
	});

	it('selects an edge when no file is selected', () => {
		const list = { fileCount: 5, selectedAnchor: -1 };
		expect(interpretFileListKeyDown(keyEvent('ArrowDown'), list)).toEqual({
			type: 'navigate',
			index: 0,
		});
		expect(interpretFileListKeyDown(keyEvent('ArrowUp'), list)).toEqual({
			type: 'navigate',
			index: 4,
		});
		expect(interpretFileListKeyDown(keyEvent('End'), list)).toEqual({ type: 'navigate', index: 4 });
	});

	it('leaves arrow keys in a text input alone', () => {
		const event = keyEvent('ArrowDown', { target: document.createElement('input') });
		expect(interpretFileListKeyDown(event, { fileCount: 5, selectedAnchor: -1 })).toBeNull();
	});

	it('maps select-all and Escape', () => {
		const list = { fileCount: 5, selectedAnchor: 0 };
		expect(interpretFileListKeyDown(keyEvent('a', { metaKey: true }), list)).toEqual({
			type: 'selectAll',
		});
		expect(interpretFileListKeyDown(keyEvent('Escape'), list)).toEqual({ type: 'clear' });
	});
});
