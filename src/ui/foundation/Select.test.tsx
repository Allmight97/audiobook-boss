import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { createSignal } from 'solid-js';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { Select } from './Select';

describe('Select', () => {
	afterEach(() => cleanup());

	function choose(value: string): void {
		const select = screen.getByRole<HTMLSelectElement>('combobox');
		select.value = value;
		fireEvent.change(select);
	}

	it('shows the engine value again when the choice changes nothing', async () => {
		const onChoose = vi.fn();
		render(() => (
			<Select value="a" onChoose={onChoose}>
				<option value="a">A</option>
				<option value="b">B</option>
			</Select>
		));

		choose('b');

		await vi.waitFor(() => expect(screen.getByRole('combobox')).toHaveValue('a'));
		expect(onChoose).toHaveBeenCalledWith('b');
	});

	it('waits for an async choice, then shows the value the engine holds', async () => {
		const [held, setHeld] = createSignal('a');
		let finish: () => void = () => undefined;
		render(() => (
			<Select
				value={held()}
				onChoose={() =>
					new Promise<void>((resolve) => {
						finish = () => {
							setHeld('b');
							resolve();
						};
					})
				}
			>
				<option value="a">A</option>
				<option value="b">B</option>
			</Select>
		));

		choose('b');
		expect(screen.getByRole('combobox')).toHaveValue('b');
		finish();

		await vi.waitFor(() => expect(screen.getByRole('combobox')).toHaveValue('b'));
		expect(held()).toBe('b');
	});
});
