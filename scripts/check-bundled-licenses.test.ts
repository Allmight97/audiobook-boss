import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { afterEach, describe, expect, it } from 'vitest';

import {
	bundleResourceProblems,
	bundleResources,
	ffmpegNoticeProblem,
} from './check-bundled-licenses';

describe('bundled licenses', () => {
	const scratch: string[] = [];
	afterEach(() => {
		for (const dir of scratch.splice(0)) rmSync(dir, { recursive: true, force: true });
	});

	function layout() {
		const root = mkdtempSync(path.join(os.tmpdir(), 'abb-release-licenses-'));
		scratch.push(root);
		mkdirSync(path.join(root, 'src-tauri'));
		mkdirSync(path.join(root, 'vendor/lib'), { recursive: true });
		writeFileSync(path.join(root, 'vendor/lib/COPYING'), 'license text');
		writeFileSync(path.join(root, 'vendor/lib/NOTICE.md'), 'notice text');
		const resourcesDir = path.join(root, 'App.app/Contents/Resources');
		mkdirSync(path.join(resourcesDir, 'licenses'), { recursive: true });
		const resources = bundleResources({
			bundle: {
				resources: {
					'../vendor/lib/COPYING': 'licenses/Lib.txt',
					'../vendor/lib/NOTICE.md': 'licenses/Lib-source.md',
				},
			},
		});
		const check = () =>
			bundleResourceProblems(resources, path.join(root, 'src-tauri'), resourcesDir);
		return { resourcesDir, check };
	}

	it('passes when every listed resource ships with its source bytes', () => {
		const { resourcesDir, check } = layout();
		writeFileSync(path.join(resourcesDir, 'licenses/Lib.txt'), 'license text');
		writeFileSync(path.join(resourcesDir, 'licenses/Lib-source.md'), 'notice text');
		expect(check()).toEqual([]);
	});

	it('names a missing resource and one that differs from its source', () => {
		const { resourcesDir, check } = layout();
		writeFileSync(path.join(resourcesDir, 'licenses/Lib-source.md'), 'stale notice');
		expect(check()).toEqual([
			'licenses/Lib.txt is missing (from ../vendor/lib/COPYING)',
			'licenses/Lib-source.md differs from ../vendor/lib/NOTICE.md',
		]);
	});

	it('names a shipped FFmpeg notice that lacks the pinned commit', () => {
		const { resourcesDir } = layout();
		const resources: [string, string][] = [
			['../vendor/ffmpeg-sys-next-9.0.1/ABB-PROVENANCE.md', 'licenses/FFmpeg-source.md'],
		];
		const pinned = 'f'.repeat(40);
		writeFileSync(path.join(resourcesDir, 'licenses/FFmpeg-source.md'), `commit \`${pinned}\``);
		expect(ffmpegNoticeProblem(resources, resourcesDir, pinned)).toBeUndefined();
		expect(ffmpegNoticeProblem(resources, resourcesDir, 'e'.repeat(40))).toBe(
			`licenses/FFmpeg-source.md does not name the pinned FFmpeg commit ${'e'.repeat(40)}`,
		);
		expect(ffmpegNoticeProblem([], resourcesDir, pinned)).toContain('ships no');
	});

	it('refuses a resources list without targets', () => {
		expect(() => bundleResources({ bundle: { resources: ['../COPYING'] } })).toThrow(
			'must map each source to its target',
		);
	});
});
