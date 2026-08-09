import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

describe('app:dev:log entrypoint', () => {
	it('builds Tauri development with the bundled FFmpeg feature', () => {
		const script = readFileSync(path.resolve(process.cwd(), 'scripts/dev-tauri-log.sh'), 'utf8');

		expect(script).toContain('bun run tauri dev --features bundled-ffmpeg');
	});

	it('generates bindings against the supported bundled FFmpeg build', () => {
		const packageJson = JSON.parse(
			readFileSync(path.resolve(process.cwd(), 'package.json'), 'utf8'),
		) as { scripts: Record<string, string> };

		expect(packageJson.scripts['bindings:generate']).toContain('--features bundled-ffmpeg');
	});
});
