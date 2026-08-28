import { cleanup, render, screen } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createAppRuntime, type AppRuntime } from '../../app/runtime';
import { inputCapabilityAtom } from '../../app/inputSession/atoms';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { App } from '../App';

const metadata: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['m4b'],
	formatsText: 'M4B',
	supportText: 'Supports M4B audio files',
};

function analyzedFile(path: string): FileListInfo {
	return {
		files: [
			{
				path,
				isValid: true,
				duration: 300,
				size: 15 * 1024 * 1024,
				format: 'm4b',
				tagTitle: 'Chapter One',
				tagArtist: 'Narrator',
				inputId: 'input-1',
			},
		],
		selectedDecoders: [null],
		totalDuration: 300,
		totalSize: 15 * 1024 * 1024,
		validCount: 1,
		invalidCount: 0,
	};
}

function fakeInput(): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/chapter.m4b']),
		openDirectory: vi.fn(async () => null),
		discoverAudioImportPaths: vi.fn(async (paths) => [...paths]),
		analyzeAudioFiles: vi.fn(async () => analyzedFile('/books/chapter.m4b')),
		getSupportedAudioImportMetadata: vi.fn(async () => metadata),
		takeOpenedAudioFiles: vi.fn(async () => []),
	};
}

describe('Solid import tracer shell', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	it('renders an analyzed local import row from picker intent', async () => {
		const user = userEvent.setup();
		runtime = createAppRuntime();
		runtime.registry.set(inputCapabilityAtom, fakeInput());
		render(() => (
			<AppRuntimeProvider registry={runtime!.registry}>
				<App />
			</AppRuntimeProvider>
		));

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		expect(await screen.findByRole('option', { name: 'Chapter One' })).toBeInTheDocument();
		expect(screen.getByText(/Narrator/)).toBeInTheDocument();
		expect(screen.getByRole('region', { name: 'Input and File Order' })).toBeInTheDocument();
		expect(screen.queryByRole('region', { name: 'Metadata Manager' })).toBeNull();
	});
});
