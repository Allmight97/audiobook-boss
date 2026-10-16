import { describe, expect, it } from 'vitest';
import { lanesRun, unmetLanes } from './verification';

const lanes = ['frontend', 'core', 'engine', 'tooling'];
const map = {
	frontend: ['src/**'],
	shared: ['Cargo.toml'],
	core: ['crates/*/src/**'],
	engine: ['crates/**'],
	tooling: ['docs/**'],
	'no-lane': ['LICENSE'],
};

describe('unmetLanes', () => {
	it('rejects an unmapped file instead of reporting that no proof is needed', () => {
		expect(() => unmetLanes(map, lanes, { changed: ['future/worker.rs'], passes: [] })).toThrow(
			/future\/worker.rs/,
		);
	});

	it('names every lane a changed file needs when nothing passed', () => {
		expect(unmetLanes(map, lanes, { changed: ['crates/abb-engine/src/a.rs'], passes: [] })).toEqual(
			['core', 'engine'],
		);
	});

	it('does not accept a frontend run as proof for a Rust edit', () => {
		const passes = [{ command: 'bash scripts/verify.sh frontend', changed: [] }];
		expect(unmetLanes(map, lanes, { changed: ['crates/abb-engine/src/a.rs'], passes })).toEqual([
			'core',
			'engine',
		]);
	});

	it('needs the lane again after a file it covers changes', () => {
		const passes = [
			{ command: 'bash scripts/verify.sh core engine', changed: ['crates/abb-engine/src/a.rs'] },
		];
		expect(unmetLanes(map, lanes, { changed: ['crates/abb-engine/src/a.rs'], passes })).toEqual([
			'core',
			'engine',
		]);
	});

	it('keeps a lane met when a later edit is outside it', () => {
		const passes = [{ command: 'bash scripts/verify.sh core engine', changed: ['docs/notes.md'] }];
		expect(
			unmetLanes(map, lanes, { changed: ['crates/abb-engine/src/a.rs', 'docs/notes.md'], passes }),
		).toEqual(['tooling']);
	});

	it('ignores map keys that are not verify.sh lanes', () => {
		expect(unmetLanes(map, lanes, { changed: ['Cargo.toml', 'LICENSE'], passes: [] })).toEqual([]);
	});
});

describe('lanesRun', () => {
	it('reads named lanes and treats no lane as every default lane', () => {
		expect(lanesRun('bash scripts/verify.sh engine host', lanes)).toEqual(['engine', 'host']);
		expect(lanesRun('scripts/verify.sh', lanes)).toEqual(lanes);
	});

	it('does not count a piped run, whose exit code is not verify.sh', () => {
		expect(lanesRun('bash scripts/verify.sh engine | tail -5', lanes)).toEqual([]);
	});
});
