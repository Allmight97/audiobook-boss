import { describe, expect, it } from 'vitest';
import { checkGuidance, type GuidanceContext, isGuidanceFile } from './check-guidance';

const context: GuidanceContext = {
	packageScripts: new Set(['app:dev:log', 'test']),
	trackedTopLevel: new Set(['scripts', 'src-tauri', 'src']),
	pathExists: (repoRelativePath) =>
		['scripts/verify.sh', 'src-tauri/AGENTS.md', 'crates/x/src/lib.rs'].includes(repoRelativePath),
};

function messages(text: string, file = 'scripts/AGENTS.md'): string[] {
	return checkGuidance(file, text, context).map((finding) => `${finding.line}: ${finding.message}`);
}

describe('check-guidance', () => {
	it('selects agent guidance, not upstream or history files', () => {
		expect(isGuidanceFile('crates/abb-engine/AGENTS.md')).toBe(true);
		expect(isGuidanceFile('.agents/skills/release/references/execution.md')).toBe(true);
		expect(isGuidanceFile('README.md')).toBe(true);
		expect(isGuidanceFile('CHANGELOG.md')).toBe(false);
		expect(isGuidanceFile('vendor/ffmpeg-sys-next-9.0.0/README.md')).toBe(false);
	});

	it('passes guidance that names lanes and scripts', () => {
		expect(messages('Run `bash scripts/verify.sh engine`, then `bun run app:dev:log`.')).toEqual(
			[],
		);
	});

	// d4afeb48^:scripts/AGENTS.md:41 copied workspace Clippy flags that went stale.
	it('rejects a copied --features string', () => {
		expect(
			messages(
				'`--features audiobook-boss/bundled-ffmpeg-portable,abb-engine/bundled-ffmpeg-portable`.',
			),
		).toEqual([expect.stringMatching(/^1: .*copies a command that a script owns/)]);
	});

	it('rejects cargo commands in inline code and in fenced blocks', () => {
		expect(messages('Loop: `cargo test --locked -p abb-media-core`.')).toHaveLength(1);
		expect(messages('```bash\ncargo clippy -p abb-engine\n```')).toEqual([
			expect.stringMatching(/^2: .*copies a command/),
		]);
	});

	// 4af7bdcb:scripts/AGENTS.md:137 named `app:dev`, which package.json never had.
	it('rejects a package script that does not exist, bare or after bun run', () => {
		expect(messages('including `bun run bindings:generate` and `app:dev`.')).toEqual([
			expect.stringMatching(/`bindings:generate` is not a package.json script/),
			expect.stringMatching(/`app:dev` is not a package.json script/),
		]);
	});

	// a7853682^:scripts/AGENTS.md:101 pointed at a folder that had moved.
	it('rejects a repo path that resolves nowhere, from the root or the file', () => {
		expect(messages('Rules: `src-tauri/src/commands/AGENTS.md` + `src-tauri/AGENTS.md`.')).toEqual([
			expect.stringMatching(
				/`src-tauri\/src\/commands\/AGENTS.md` names a path that does not exist/,
			),
		]);
		expect(messages('See `src/lib.rs`.', 'crates/x/AGENTS.md')).toEqual([]);
	});

	it('ignores paths outside the repo and placeholders', () => {
		expect(
			messages(
				'`~/.local/bin`, `.logs/runs/<run-id>/`, `target/abb-ffmpeg-cache/`, `/Applications/X.app`.',
			),
		).toEqual([]);
	});

	it('rejects a copy of the CI trigger events', () => {
		expect(messages('CI runs on `opened`, `ready_for_review`, and `auto_merge_enabled`.')).toEqual([
			expect.stringMatching(/fact owned by `.github\/workflows\/ci.yml`/),
		]);
	});
});
