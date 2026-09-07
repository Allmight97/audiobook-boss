import { createContext, useContext } from 'solid-js';
import type { JSX } from '@solidjs/web';

import type { AppRuntime } from './types';

const AppRuntimeContext = createContext<AppRuntime>();

export function AppRuntimeProvider(props: {
	readonly runtime: AppRuntime;
	readonly children: JSX.Element;
}): JSX.Element {
	return <AppRuntimeContext value={props.runtime}>{props.children}</AppRuntimeContext>;
}

export function useAppRuntime(): AppRuntime {
	try {
		return useContext(AppRuntimeContext);
	} catch {
		throw new Error('App runtime is not mounted');
	}
}
