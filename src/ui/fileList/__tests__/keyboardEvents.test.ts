import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import { setCurrentFileList, setSelectedFileIndices, setSelectedIndex } from '../state';

const context = vi.hoisted(() => ({
	clearSelectionActionMock: vi.fn(),
	moveFileDownMock: vi.fn(),
	moveFileUpMock: vi.fn(),
	removeFileMock: vi.fn(),
	reorderFilesMock: vi.fn(),
	selectAllMock: vi.fn(),
	selectFileMock: vi.fn(async () => undefined),
}));

vi.mock('../actions', () => ({
	clearSelectionAction: context.clearSelectionActionMock,
	moveFileDown: context.moveFileDownMock,
	moveFileUp: context.moveFileUpMock,
	removeFile: context.removeFileMock,
	reorderFiles: context.reorderFilesMock,
	selectAll: context.selectAllMock,
	selectFile: context.selectFileMock,
}));

const makeFileList = (count: number): FileListInfo => {
	const files: AudioFile[] = Array.from({ length: count }, (_, index) => ({
		path: `/books/book-${index}.m4b`,
		isValid: true,
		size: 1,
		duration: 1,
		bitrate: 64,
		sampleRate: 44_100,
		channels: 2,
	}));

	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: count,
		totalSize: count,
		validCount: count,
		invalidCount: 0,
	};
};

type KeyboardEventOptions = Partial<
	Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'shiftKey' | 'altKey'>
> & {
	target?: EventTarget;
};

function keyboardEvent(key: string, options: KeyboardEventOptions = {}): KeyboardEvent {
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

describe('file list keyboard events', () => {
	beforeEach(() => {
		setCurrentFileList(makeFileList(5));
		setSelectedIndex(-1);
		setSelectedFileIndices([]);
		context.clearSelectionActionMock.mockClear();
		context.moveFileDownMock.mockClear();
		context.moveFileUpMock.mockClear();
		context.removeFileMock.mockClear();
		context.reorderFilesMock.mockClear();
		context.selectAllMock.mockClear();
		context.selectFileMock.mockClear();
	});

	it('selects the first file from an empty selection on ArrowDown', async () => {
		const { onFileListKeyDown } = await import('../events');
		const event = keyboardEvent('ArrowDown');

		onFileListKeyDown(event);

		expect(event.preventDefault).toHaveBeenCalled();
		expect(context.selectFileMock).toHaveBeenCalledWith(0, { multi: false, range: false });
	});

	it('jumps to the bottom of the file list on End', async () => {
		const { onFileListKeyDown } = await import('../events');
		setSelectedIndex(1);
		setSelectedFileIndices([1]);
		const event = keyboardEvent('End');

		onFileListKeyDown(event);

		expect(event.preventDefault).toHaveBeenCalled();
		expect(context.selectFileMock).toHaveBeenCalledWith(4, { multi: false, range: false });
	});

	it('does not reselect while already colliding with a list edge', async () => {
		const { onFileListKeyDown } = await import('../events');
		setSelectedIndex(4);
		setSelectedFileIndices([4]);
		const event = keyboardEvent('ArrowDown');

		onFileListKeyDown(event);

		expect(event.preventDefault).toHaveBeenCalled();
		expect(context.selectFileMock).not.toHaveBeenCalled();
	});

	it('leaves text input arrow handling alone', async () => {
		const { onFileListKeyDown } = await import('../events');
		const input = document.createElement('input');
		const event = keyboardEvent('ArrowDown', { target: input });

		onFileListKeyDown(event);

		expect(event.preventDefault).not.toHaveBeenCalled();
		expect(context.selectFileMock).not.toHaveBeenCalled();
	});

	it('keeps existing file-list select all behavior inside the focused region', async () => {
		const { onFileListKeyDown } = await import('../events');
		const event = keyboardEvent('a', { metaKey: true });

		onFileListKeyDown(event);

		expect(event.preventDefault).toHaveBeenCalled();
		expect(context.selectAllMock).toHaveBeenCalledTimes(1);
	});
});
