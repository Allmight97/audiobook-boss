import { cleanup, render, screen, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SupportedAudioImportMetadata } from '../../types/audio';
import { audioFile, createFakeEngine } from '../../test/fixtures/fakeEngine';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createAppRuntime, type AppRuntime } from '../../app/runtime';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { App } from '../App';

const metadata: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['m4b'],
	formatsText: 'M4B',
	supportText: 'Supports M4B audio files',
};

function fakeInput(): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/chapter.m4b']),
		openDirectory: vi.fn(async () => null),
		getSupportedAudioImportMetadata: vi.fn(async () => metadata),
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
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
		const engine = createFakeEngine();
		engine.analyze = (paths) =>
			paths.map((path) =>
				audioFile(path, {
					inputId: 'input-1',
					duration: 300,
					size: 15 * 1024 * 1024,
					tagTitle: 'Chapter One',
					tagArtist: 'Narrator',
				}),
			);
		runtime = createAppRuntime({ input: fakeInput(), engine });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<App />
			</AppRuntimeProvider>
		));
		await screen.findByTestId('left-column');

		await user.click(screen.getByRole('button', { name: 'Add audio files' }));
		const row = await screen.findByRole('option', { name: 'chapter.m4b' });
		expect(within(row).getByText('Chapter One')).toBeInTheDocument();
		expect(document.querySelector('.file-details')?.textContent).toMatch(/Narrator/);
		expect(screen.getByRole('region', { name: 'Input and File Order' })).toBeInTheDocument();
		expect(screen.getByRole('region', { name: 'Selected File Properties' })).toBeInTheDocument();
		expect(screen.getByRole('region', { name: 'Metadata Manager' })).toBeInTheDocument();
	});
});
