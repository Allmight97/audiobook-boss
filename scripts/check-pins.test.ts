import { describe, expect, it } from 'vitest';
import { checkPins } from './check-pins';

const TODAY = '2026-10-26';
const TAGGED = 'pin: why; unpin when specta is stable';
const row = (file: string, name: string, tag: string) => `| \`${file}\` | \`${name}\` | ${tag} |`;

function run(files: Record<string, string>, agentsMd = '') {
	return checkPins({ files, agentsMd }, TODAY).map(
		(finding) => `${finding.where} ${finding.message}`,
	);
}

describe('check-pins', () => {
	it('accepts a Cargo pin tagged on the line above, shared down the block', () => {
		const cargo = `# ${TAGGED}\nspecta = "=2.0.0-rc.25"\nspecta-typescript = "=0.0.12"\n`;
		expect(run({ 'crates/x/Cargo.toml': cargo })).toEqual([]);
	});

	it('rejects an untagged Cargo pin and a tag a blank line away', () => {
		const cargo = `# ${TAGGED}\n\nspecta = { version = "=2.0.0-rc.25", features = ["derive"] }\nserde = "1"\n`;
		expect(run({ 'crates/x/Cargo.toml': cargo })).toEqual([
			expect.stringContaining('crates/x/Cargo.toml: specta has no'),
		]);
	});

	it('rejects a tag whose unpin date has passed', () => {
		const cargo = `# pin: held; unpin when 2026-10-20\nrsa = "=0.9.10"\n`;
		expect(run({ 'crates/x/Cargo.toml': cargo })).toEqual([
			expect.stringContaining('unpin date 2026-10-20 has passed'),
		]);
		expect(run({ 'crates/x/Cargo.toml': cargo.replace('10-20', '10-26') })).toEqual([]);
	});

	it('requires a tag for each bunfig exclusion', () => {
		const bunfig = `minimumReleaseAgeExcludes = [\n  # ${TAGGED}\n  "solid-js",\n  "@solidjs/web",\n\n  "seroval",\n]\n`;
		expect(run({ 'bunfig.toml': bunfig })).toEqual([
			expect.stringContaining('bunfig.toml: seroval has no'),
		]);
	});

	it('takes package.json exact versions and overrides from the table', () => {
		const pkg = JSON.stringify({
			dependencies: { 'solid-js': '2.0.0-rc.15', other: '^1.0.0' },
			devDependencies: { typescript: 'npm:typescript@7.0.2' },
			overrides: { undici: '^7.30.1' },
		});
		expect(run({ 'package.json': pkg })).toHaveLength(3);
		const table = [
			row('package.json', 'solid-js', TAGGED),
			row('package.json', 'typescript', TAGGED),
			row('package.json', 'undici (override)', TAGGED),
		].join('\n');
		expect(run({ 'package.json': pkg }, table)).toEqual([]);
	});

	it('rejects a table row for a pin that no longer exists', () => {
		expect(run({ 'package.json': '{}' }, row('package.json', 'gone', TAGGED))).toEqual([
			expect.stringContaining('package.json: gone has a pin table row but no such pin'),
		]);
	});

	it('covers .node-version, the Rust channel, and uvx pins', () => {
		const files = {
			'.node-version': '26.12.0\n',
			'rust-toolchain.toml': '[toolchain]\nchannel = "1.99"\n',
			'scripts/x.sh':
				'uvx --from "zizmor==1.30.1" zizmor\nuvx --exclude-newer-package "lizard=2026-10-06" lizard\n',
		};
		expect(run(files)).toHaveLength(4);
	});
});
