import { convertFileSrc } from '@tauri-apps/api/core';

/** A cover a view shows, as the engine names it. */
export type CoverSource =
	| { readonly kind: 'remote'; readonly url: string; readonly size: 'small' | 'full' }
	/** `revision` is the titles part's `coversRevision`. */
	| { readonly kind: 'audio'; readonly path: string; readonly revision: number }
	/** The cover the metadata form shows, at the metadata part's `imageRevision`. */
	| { readonly kind: 'session'; readonly revision: number }
	| { readonly kind: 'preview'; readonly runId: string };

const SCHEME = 'abb-cover';

/**
 * The address an `<img>` loads `source` from. The engine serves it through
 * the host's `abb-cover` scheme; an address names one image, so the webview
 * may cache it.
 */
export function coverSrc(source: CoverSource): string {
	const [size, revision, value] = parts(source);
	return convertFileSrc(`${source.kind}/${size}/${revision}/${encodeURIComponent(value)}`, SCHEME);
}

function parts(source: CoverSource): [size: string, revision: number, value: string] {
	switch (source.kind) {
		case 'remote':
			return [source.size, 0, source.url];
		case 'audio':
			return ['small', source.revision, source.path];
		case 'session':
			return ['full', source.revision, ''];
		case 'preview':
			return ['small', 0, source.runId];
	}
}
