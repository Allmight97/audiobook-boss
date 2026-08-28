import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import type { AudioFile, FileListInfo } from '../../../../types/audio';
import { FileListIsland, loadFileList, resetFileList, selectFile, setOrderLocked } from '..';

function makeFileList(): FileListInfo {
	const files: AudioFile[] = [
		{ path: '/books/alpha.m4b', isValid: true, duration: 60, size: 1024, format: 'm4b' },
		{ path: '/books/bravo.m4b', isValid: true, duration: 60, size: 2048, format: 'm4b' },
	];
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: 120,
		totalSize: 3072,
		validCount: files.length,
		invalidCount: 0,
	};
}

describe('Solid 2 File List island', () => {
	beforeEach(() => {
		resetFileList();
		loadFileList(makeFileList());
	});

	afterEach(() => {
		cleanup();
		resetFileList();
	});

	it('handles keyboard actions only from the focusable FileList region', async () => {
		render(() => <FileListIsland />);
		const listbox = screen.getByRole('listbox', { name: 'Audio files' });

		await fireEvent.keyDown(listbox, { key: 'ArrowDown' });
		expect(screen.getByRole('option', { name: 'alpha.m4b' })).toHaveAttribute(
			'aria-selected',
			'true',
		);

		await fireEvent.keyDown(document.body, { key: 'ArrowDown' });
		await fireEvent.keyDown(window, { key: 'ArrowDown' });
		expect(screen.getByRole('option', { name: 'bravo.m4b' })).toHaveAttribute(
			'aria-selected',
			'false',
		);

		await fireEvent.keyDown(listbox, { key: 'a', ctrlKey: true });
		expect(screen.getByRole('option', { name: 'alpha.m4b' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
		expect(screen.getByRole('option', { name: 'bravo.m4b' })).toHaveAttribute(
			'aria-selected',
			'true',
		);
	});

	it('paints inspector and sort from Solid view state', async () => {
		selectFile(0);
		render(() => <FileListIsland />);
		expect(screen.getByTestId('inspector-context')).toHaveTextContent('alpha.m4b');
		expect(screen.getByTestId('inspector-detail')).toHaveTextContent('1 of 2');
		expect(screen.getByTestId('file-order-lock')).not.toBeVisible();

		await fireEvent.click(screen.getByRole('button', { name: /Sort files/ }));
		expect(screen.getByLabelText(/Sort files descending/)).toHaveTextContent('Sort: A-Z');

		setOrderLocked(true);
		expect(screen.getByTestId('file-order-lock')).toBeVisible();
		expect(screen.getByRole('button', { name: 'Clear' })).toBeDisabled();
	});
});
