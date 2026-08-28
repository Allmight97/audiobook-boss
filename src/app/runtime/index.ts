import { AtomRegistry } from './reactivity';

export type AppRuntime = {
	readonly registry: AtomRegistry.AtomRegistry;
	dispose(): void;
};

export function createAppRuntime(): AppRuntime {
	const registry = AtomRegistry.make();
	return {
		registry,
		dispose(): void {
			registry.dispose();
		},
	};
}
