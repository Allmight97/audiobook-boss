import { screen, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { MetadataField } from '../../lib/generated/tauri';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { renderApp } from '../../test/fixtures/renderApp';

const pickAlpha = { openFiles: vi.fn(async () => ['/books/alpha.m4b']) };

function engineWithTaggedBooks({ cover = true } = {}): FakeEngine {
	const engine = createFakeEngine();
	const form: Partial<Record<MetadataField, string>> = {};
	engine.analyze = (paths) =>
		paths.map((path) => audioFile(path, { status: 'ready', label: 'Alpha' }));
	engine.respond = (intent) => {
		if (intent.kind === 'setField') {
			form[intent.field] = intent.value;
			engine.seedForm(form);
		}
		if (intent.kind !== 'import') return undefined;
		engine.loadTitles(engine.analyze(intent.paths), [0]);
		form.title = 'Alpha';
		engine.seedForm(form);
		if (cover)
			engine.change((state) => {
				state.metadata.cover.image = 'session/1/';
			});
		return { kind: 'applied' };
	};
	return engine;
}

function lastIntents(engine: FakeEngine, count: number) {
	return engine.sessionIntents.slice(-count);
}

describe('metadata workbench shell', () => {
	it.each([
		{ name: 'while a cover shows', cover: true, tabIndex: 0 },
		{ name: 'not when no cover shows', cover: false, tabIndex: -1 },
	])('lets Tab reach Clear cover $name', async ({ cover, tabIndex }) => {
		const engine = engineWithTaggedBooks({ cover });
		await renderApp({ engine, input: pickAlpha });
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement | null)?.value).toBe(
				'Alpha',
			);
		});
		expect((document.getElementById('cover-art-clear-btn') as HTMLButtonElement).tabIndex).toBe(
			tabIndex,
		);
	});

	it('sends a typed title and the save to the engine in that order', async () => {
		const engine = engineWithTaggedBooks();
		await renderApp({ engine, input: pickAlpha });
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		title.focus();
		await userEvent.clear(title);
		await userEvent.type(title, 'Edited');
		engine.respond = (intent) => {
			if (intent.kind !== 'save') return undefined;
			engine.status({
				kind: 'saveComplete',
				succeeded: 1,
				failed: 0,
				cancelled: 0,
				waiting: 0,
				held: 0,
				outputs: { updated: 0, elsewhere: 0, restartOffered: 0, failed: 0 },
			});
			return { kind: 'applied' };
		};
		await userEvent.click(screen.getByTestId('metadata-save-btn'));
		await waitFor(() => expect(lastIntents(engine, 2)[1]).toEqual({ kind: 'save' }));
		expect(lastIntents(engine, 2)[0]).toEqual({
			kind: 'setField',
			binding: 0,
			field: 'title',
			value: 'Edited',
		});
		expect(title.value).toBe('Edited');
		expect(screen.getByTestId('metadata-status-message')).toHaveTextContent(
			'Metadata save complete: success=1, failed=0, cancelled=0',
		);
	});

	it('sends each form control to the engine field it edits', async () => {
		const engine = engineWithTaggedBooks();
		await renderApp({ engine, input: pickAlpha });
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		engine.respond = () => undefined;
		engine.change((state) => {
			state.metadata.form = { ...state.metadata.form, mode: 'multi' };
		});

		await userEvent.type(document.getElementById('meta-year') as HTMLInputElement, '2');
		await userEvent.type(document.getElementById('meta-description') as HTMLTextAreaElement, 'D');
		await userEvent.selectOptions(await screen.findByTestId('meta-series-part-action'), 'blank');

		await waitFor(() =>
			expect(lastIntents(engine, 3)).toEqual([
				{ kind: 'setField', binding: 0, field: 'date', value: '2' },
				{ kind: 'setField', binding: 0, field: 'description', value: 'D' },
				{ kind: 'setFieldAction', binding: 0, field: 'seriesPart', action: 'blank' },
			]),
		);
	});

	it('saves from the global shortcut', async () => {
		const engine = engineWithTaggedBooks();
		await renderApp({ engine, input: pickAlpha });
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		await userEvent.type(title, ' Two');
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 's', metaKey: true, bubbles: true }));
		await waitFor(() =>
			expect(lastIntents(engine, 2)).toEqual([
				{ kind: 'setField', binding: 0, field: 'title', value: 'Alpha Two' },
				{ kind: 'save' },
			]),
		);
	});
});
