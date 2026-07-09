import { SvelteSet } from 'svelte/reactivity';

export type SelectionIntent =
	| { type: 'selectOnly'; id: number }
	| { type: 'toggle'; id: number }
	| { type: 'selectAll' }
	| { type: 'clear' }
	| { type: 'replacePreset'; ids: readonly number[] };

export type PrototypeSelectionOptions = {
	validIds: readonly number[];
	initialIds: readonly number[];
	initialActiveId?: number | null;
};

export function createPrototypeSelection(options: PrototypeSelectionOptions) {
	const validIds = new Set(options.validIds);
	const selectedIds = new SvelteSet<number>(options.initialIds.filter((id) => validIds.has(id)));
	let activeId = $state<number | null>(options.initialActiveId ?? options.initialIds[0] ?? null);

	function repairActiveId(): void {
		if (activeId !== null && !selectedIds.has(activeId)) {
			const first = selectedIds.values().next().value;
			activeId = first ?? null;
		}
	}

	function apply(intent: SelectionIntent): void {
		switch (intent.type) {
			case 'selectOnly': {
				if (!validIds.has(intent.id)) return;
				selectedIds.clear();
				selectedIds.add(intent.id);
				activeId = intent.id;
				break;
			}
			case 'toggle': {
				if (!validIds.has(intent.id)) return;
				if (selectedIds.has(intent.id)) {
					selectedIds.delete(intent.id);
					repairActiveId();
				} else {
					selectedIds.add(intent.id);
					activeId = intent.id;
				}
				break;
			}
			case 'selectAll': {
				selectedIds.clear();
				for (const id of validIds) {
					selectedIds.add(id);
				}
				if (activeId === null || !selectedIds.has(activeId)) {
					activeId = options.validIds[0] ?? null;
				}
				break;
			}
			case 'clear': {
				selectedIds.clear();
				activeId = null;
				break;
			}
			case 'replacePreset': {
				selectedIds.clear();
				for (const id of intent.ids) {
					if (validIds.has(id)) {
						selectedIds.add(id);
					}
				}
				repairActiveId();
				break;
			}
		}
	}

	return {
		get selectedIds() {
			return selectedIds;
		},
		get activeId() {
			return activeId;
		},
		setActiveId(id: number | null): void {
			if (id === null || selectedIds.has(id)) {
				activeId = id;
			}
		},
		get count() {
			return selectedIds.size;
		},
		isSelected(id: number): boolean {
			return selectedIds.has(id);
		},
		isMulti(): boolean {
			return selectedIds.size > 1;
		},
		isAllSelected(): boolean {
			return selectedIds.size === validIds.size;
		},
		isIndeterminate(): boolean {
			return selectedIds.size > 0 && selectedIds.size < validIds.size;
		},
		apply,
	};
}

export type PrototypeSelection = ReturnType<typeof createPrototypeSelection>;
