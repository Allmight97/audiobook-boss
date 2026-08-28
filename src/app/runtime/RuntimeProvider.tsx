import type { JSX } from 'solid-js';
import { onCleanup } from 'solid-js';
import { createAppRuntime } from './index';
import type { AtomRegistry } from './reactivity';
import { RegistryContext } from './solid';

export function AppRuntimeProvider(props: {
	readonly registry: AtomRegistry.AtomRegistry;
	readonly children: JSX.Element;
}): JSX.Element {
	return (
		<RegistryContext.Provider value={props.registry}>{props.children}</RegistryContext.Provider>
	);
}

export function AppRoot(props: { readonly children: JSX.Element }): JSX.Element {
	const runtime = createAppRuntime();
	onCleanup(() => runtime.dispose());
	return <AppRuntimeProvider registry={runtime.registry}>{props.children}</AppRuntimeProvider>;
}
