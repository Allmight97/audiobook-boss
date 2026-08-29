import type { EncodingRequestConfig } from '../../types/audio';
import { Atom } from '../../app/runtime/reactivity';
import { readEncodingRequestConfig } from './state.svelte';

export const encodingRequestConfigAtom = Atom.make<EncodingRequestConfig>(
	readEncodingRequestConfig(),
).pipe(Atom.keepAlive);

export function publishEncodingRequestConfig(
	set: (config: EncodingRequestConfig) => void,
): void {
	set(readEncodingRequestConfig());
}
