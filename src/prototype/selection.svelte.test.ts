import { describe, expect, it } from 'vitest';
import { createPrototypeSelection } from './selection.svelte';

const VALID_IDS = [0, 1, 2, 3, 4] as const;

describe('PrototypeSelection', () => {
	it('starts with valid initial selection and active id', () => {
		const selection = createPrototypeSelection({
			validIds: VALID_IDS,
			initialIds: [0],
			initialActiveId: 0,
		});

		expect(selection.count).toBe(1);
		expect(selection.isSelected(0)).toBe(true);
		expect(selection.activeId).toBe(0);
		expect(selection.isMulti()).toBe(false);
	});

	it('selectOnly replaces the selection set', () => {
		const selection = createPrototypeSelection({
			validIds: VALID_IDS,
			initialIds: [0, 1, 2],
			initialActiveId: 0,
		});

		selection.apply({ type: 'selectOnly', id: 3 });

		expect(selection.count).toBe(1);
		expect(selection.isSelected(3)).toBe(true);
		expect(selection.activeId).toBe(3);
	});

	it('toggle adds and removes ids while repairing active id', () => {
		const selection = createPrototypeSelection({
			validIds: VALID_IDS,
			initialIds: [1],
			initialActiveId: 1,
		});

		selection.apply({ type: 'toggle', id: 2 });
		expect(selection.isSelected(2)).toBe(true);
		expect(selection.activeId).toBe(2);

		selection.apply({ type: 'toggle', id: 2 });
		expect(selection.isSelected(2)).toBe(false);
		expect(selection.activeId).toBe(1);
	});

	it('replacePreset swaps to a preset selection', () => {
		const selection = createPrototypeSelection({
			validIds: VALID_IDS,
			initialIds: [0],
			initialActiveId: 0,
		});

		selection.apply({ type: 'replacePreset', ids: [1, 2, 3] });

		expect(selection.count).toBe(3);
		expect(selection.isMulti()).toBe(true);
		expect(selection.isSelected(1)).toBe(true);
		expect(selection.isSelected(2)).toBe(true);
		expect(selection.isSelected(3)).toBe(true);
	});

	it('selectAll and clear maintain coherent active id', () => {
		const selection = createPrototypeSelection({
			validIds: VALID_IDS,
			initialIds: [2],
			initialActiveId: 2,
		});

		selection.apply({ type: 'selectAll' });
		expect(selection.isAllSelected()).toBe(true);
		expect(selection.activeId).toBe(2);

		selection.apply({ type: 'clear' });
		expect(selection.count).toBe(0);
		expect(selection.activeId).toBeNull();
	});

	it('ignores invalid ids', () => {
		const selection = createPrototypeSelection({
			validIds: VALID_IDS,
			initialIds: [0],
			initialActiveId: 0,
		});

		selection.apply({ type: 'toggle', id: 99 });
		expect(selection.count).toBe(1);
		expect(selection.isSelected(99)).toBe(false);
	});

	it('reports indeterminate select-all state', () => {
		const selection = createPrototypeSelection({
			validIds: VALID_IDS,
			initialIds: [1, 2],
			initialActiveId: 1,
		});

		expect(selection.isIndeterminate()).toBe(true);
		expect(selection.isAllSelected()).toBe(false);
	});
});
