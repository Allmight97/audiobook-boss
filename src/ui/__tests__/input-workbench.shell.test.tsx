import { cleanup, fireEvent, render, screen, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createAppRuntime, type AppRuntime } from '../../app/runtime';
import {
	importIntentAtom,
	inputCapabilityAtom,
	inputViewAtom,
	jobTypeAtom,
	setOrderLockedAtom,
} from '../../app/inputSession';
import type { InputCapability, NativeDropPayload } from '../../lib/tauri/capabilities/input';
import { App } from '../App';

const metadata: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
	formatsText: 'MP3, M4A/M4B, AAC, WAV, and FLAC',
	supportText: 'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
};

const listeners: {
	drop?: (payload: NativeDropPayload) => void;
} = {};

function analyzedFile(
	path: string,
	overrides: Record<string, unknown> = {},
): FileListInfo['files'][number] {
	return {
		path,
		isValid: true,
		duration: 1,
		size: 1000,
		format: 'mp3',
		inputId: path,
		...overrides,
	};
}

function analyzedList(files: FileListInfo['files']): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length * 1000,
		validCount: files.filter((file) => file.isValid).length,
		invalidCount: files.filter((file) => !file.isValid).length,
	};
}

function fakeInput(overrides: Partial<InputCapability> = {}): InputCapability {
	return {
		openFiles: vi.fn(async () => []),
		openDirectory: vi.fn(async () => null),
		discoverAudioImportPaths: vi.fn(async (paths: ReadonlyArray<string>) =>
			paths.filter((path) => !path.endsWith('.txt') && !path.endsWith('.png')),
		),
		analyzeAudioFiles: vi.fn(async (paths: ReadonlyArray<string>) =>
			analyzedList(paths.map((path) => analyzedFile(path))),
		),
		getSupportedAudioImportMetadata: vi.fn(async () => metadata),
		takeOpenedAudioFiles: vi.fn(async () => []),
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async (handler) => {
			listeners.drop = handler;
			return () => {
				listeners.drop = undefined;
			};
		}),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		listenOpenedAudioFiles: vi.fn(async () => () => undefined),
		...overrides,
	};
}

function renderApp(runtime: AppRuntime, input: InputCapability) {
	runtime.registry.set(inputCapabilityAtom, input);
	return render(() => (
		<AppRuntimeProvider registry={runtime.registry}>
			<App />
		</AppRuntimeProvider>
	));
}

describe('Solid input workbench', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		listeners.drop = undefined;
		document.getElementById('cover-art-area')?.remove();
	});

	it('renders input workflow before the inspector and keeps merge independent of the list', async () => {
		const user = userEvent.setup();
		runtime = createAppRuntime();
		const input = fakeInput({
			openFiles: vi.fn(async () => ['/tmp/file1.mp3']),
		});
		renderApp(runtime, input);

		const shell = screen.getByTestId('left-column');
		const workflow = screen.getByTestId('input-workflow-panel');
		const inspector = screen.getByTestId('file-inspector-panel');
		expect(workflow.parentElement).toBe(shell);
		expect(inspector.parentElement).toBe(shell);
		expect(
			workflow.compareDocumentPosition(inspector) & Node.DOCUMENT_POSITION_FOLLOWING,
		).toBeTruthy();

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		await screen.findByRole('option', { name: 'file1.mp3' });
		expect(runtime.registry.get(inputViewAtom).files).toHaveLength(1);
		expect(runtime.registry.get(jobTypeAtom)).toBe('batch');

		await user.click(screen.getByLabelText('Merge files into one audiobook'));
		expect(runtime.registry.get(jobTypeAtom)).toBe('merge');
		expect(runtime.registry.get(inputViewAtom).files).toHaveLength(1);
		expect(runtime.registry.get(inputViewAtom).selectedIndices).toEqual([0]);
	});

	it('handles keyboard actions only from the focused listbox', async () => {
		runtime = createAppRuntime();
		const input = fakeInput({
			analyzeAudioFiles: vi.fn(async () =>
				analyzedList([analyzedFile('/books/alpha.m4b'), analyzedFile('/books/bravo.m4b')]),
			),
		});
		renderApp(runtime, input);
		runtime.registry.set(importIntentAtom, {
			type: 'importPaths',
			paths: ['/books/alpha.m4b', '/books/bravo.m4b'],
		});
		await waitFor(() => {
			expect(screen.getAllByRole('option')).toHaveLength(2);
		});

		const listbox = screen.getByRole('listbox', { name: 'Audio files' });
		await fireEvent.keyDown(listbox, { key: 'ArrowDown' });
		await waitFor(() => {
			expect(runtime?.registry.get(inputViewAtom).selectedIndices).toEqual([0]);
		});

		await fireEvent.keyDown(document.body, { key: 'ArrowDown' });
		expect(runtime.registry.get(inputViewAtom).selectedIndices).toEqual([0]);

		await fireEvent.keyDown(listbox, { key: 'a', ctrlKey: true });
		await waitFor(() => {
			expect(runtime?.registry.get(inputViewAtom).selectedIndices).toEqual([0, 1]);
		});
	});

	it('routes cover-art drops away from import and imports file-area drops', async () => {
		runtime = createAppRuntime();
		const input = fakeInput();
		const cover = document.createElement('div');
		cover.id = 'cover-art-area';
		document.body.appendChild(cover);
		cover.getBoundingClientRect = () =>
			({
				left: 0,
				right: 100,
				top: 0,
				bottom: 100,
				width: 100,
				height: 100,
				x: 0,
				y: 0,
				toJSON: () => ({}),
			}) as DOMRect;

		renderApp(runtime, input);
		await waitFor(() => {
			expect(listeners.drop).toBeTypeOf('function');
		});

		const container = document.querySelector('.file-management-container') as HTMLElement;
		container.getBoundingClientRect = () =>
			({
				left: 150,
				right: 400,
				top: 150,
				bottom: 350,
				width: 250,
				height: 200,
				x: 150,
				y: 150,
				toJSON: () => ({}),
			}) as DOMRect;

		listeners.drop?.({ position: { x: 50, y: 50 }, paths: ['/tmp/image.png'] });
		await waitFor(() => {
			expect(input.analyzeAudioFiles).not.toHaveBeenCalled();
		});

		listeners.drop?.({ position: { x: 200, y: 200 }, paths: ['/tmp/file1.wav'] });
		await waitFor(() => {
			expect(input.analyzeAudioFiles).toHaveBeenCalledWith(['/tmp/file1.wav']);
		});
	});

	it('blocks import while order is locked and surfaces the lock banner', async () => {
		runtime = createAppRuntime();
		const input = fakeInput();
		renderApp(runtime, input);
		runtime.registry.set(setOrderLockedAtom, true);
		await waitFor(() => {
			expect(screen.getByTestId('file-order-lock')).toBeVisible();
		});
		runtime.registry.set(importIntentAtom, { type: 'importPaths', paths: ['/tmp/file1.mp3'] });
		await waitFor(() => {
			expect(screen.getByTestId('file-order-lock')).toBeVisible();
			expect(screen.getByText(/Wait for completion to add files/)).toBeInTheDocument();
		});
		expect(input.analyzeAudioFiles).not.toHaveBeenCalled();
	});
});
