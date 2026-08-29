import type { AtomRegistry } from '../runtime/reactivity';

let registry: AtomRegistry.AtomRegistry | null = null;

export function bindProcessingRegistry(next: AtomRegistry.AtomRegistry | null): void {
	registry = next;
}

export function tryProcessingRegistry(): AtomRegistry.AtomRegistry | null {
	return registry;
}

export function processingRegistry(): AtomRegistry.AtomRegistry {
	if (!registry) {
		throw new Error('Processing runtime is not bound');
	}
	return registry;
}
