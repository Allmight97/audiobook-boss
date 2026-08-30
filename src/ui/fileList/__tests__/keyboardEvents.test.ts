import { describe, expect, it, vi } from 'vitest';
import { interpretFileListKeyDown } from '../../../app/inputSession';

function keyboardEvent(
	key: string,
	options: Partial<Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>> & {
		target?: EventTarget;
	} = {},
): KeyboardEvent {
	return {
		key,
		ctrlKey: options.ctrlKey ?? false,
		metaKey: options.metaKey ?? false,
		shiftKey: options.shiftKey ?? false,
		altKey: options.altKey ?? false,
		target: options.target ?? document.createElement('div'),
		preventDefault: vi.fn(),
	} as unknown as KeyboardEvent;
}

describe('file list keyboard interpretation', () => {
	it('selects the first file from an empty selection on ArrowDown', () => {
		const event = keyboardEvent('ArrowDown');
		expect(interpretFileListKeyDown(event, { fileCount: 5, selectedAnchor: -1 })).toEqual({
			type: 'navigate',
			index: 0,
		});
	});

	it('jumps to the bottom of the file list on End', () => {
		const event = keyboardEvent('End');
		expect(interpretFileListKeyDown(event, { fileCount: 5, selectedAnchor: 1 })).toEqual({
			type: 'navigate',
			index: 4,
		});
	});

	it('does not change the index while already at a list edge', () => {
		expect(
			interpretFileListKeyDown(keyboardEvent('ArrowDown'), { fileCount: 5, selectedAnchor: 4 }),
		).toEqual({ type: 'navigate', index: 4 });
	});

	it('leaves text input arrow handling alone', () => {
		const event = keyboardEvent('ArrowDown', { target: document.createElement('input') });
		expect(interpretFileListKeyDown(event, { fileCount: 5, selectedAnchor: -1 })).toBeNull();
	});

	it('maps select-all and escape', () => {
		expect(
			interpretFileListKeyDown(keyboardEvent('a', { metaKey: true }), {
				fileCount: 5,
				selectedAnchor: 0,
			}),
		).toEqual({ type: 'selectAll' });
		expect(
			interpretFileListKeyDown(keyboardEvent('Escape'), { fileCount: 5, selectedAnchor: 0 }),
		).toEqual({ type: 'clear' });
	});
});
