import { describe, expect, it, vi } from 'vitest';
import { interpretFileListKeyDown } from '.';

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
		['ArrowUp', 11],
		['ArrowDown', 13],
		['Home', 0],
		['End', 24],
		['PageUp', 2],
		['PageDown', 22],
	] as const)('maps %s to a move to file %i', (key, index) => {
		expect(interpretFileListKeyDown(keyEvent(key), { fileCount: 25, selectedAnchor: 12 })).toEqual({
			type: 'navigate',
			index,
		});
	});

	it('ignores modified navigation keys', () => {
		const list = { fileCount: 25, selectedAnchor: 12 };
		expect(interpretFileListKeyDown(keyEvent('ArrowDown', { shiftKey: true }), list)).toBeNull();
		expect(interpretFileListKeyDown(keyEvent('End', { metaKey: true }), list)).toBeNull();
	});

	it('moves one file at a time and stays put at a list edge', () => {
		const list = { fileCount: 5 };
		expect(interpretFileListKeyDown(keyEvent('ArrowDown'), { ...list, selectedAnchor: 4 })).toEqual(
			{ type: 'navigate', index: 4 },
		);
		expect(interpretFileListKeyDown(keyEvent('ArrowUp'), { ...list, selectedAnchor: 0 })).toEqual({
			type: 'navigate',
			index: 0,
		});
	});

	it('jumps by a bounded page step', () => {
		const list = { fileCount: 25, selectedAnchor: 4 };
		expect(interpretFileListKeyDown(keyEvent('PageDown'), list)).toEqual({
			type: 'navigate',
			index: 14,
		});
		expect(interpretFileListKeyDown(keyEvent('PageUp'), list)).toEqual({
			type: 'navigate',
			index: 0,
		});
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
