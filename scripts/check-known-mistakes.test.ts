import { describe, expect, it } from 'vitest';
import { checkFile } from './check-known-mistakes';

function found(file: string, text: string): string[] {
	return checkFile(file, text).map((finding) => `${finding.line}: ${finding.message}`);
}

describe('check-known-mistakes', () => {
	// d7c41f03:scripts/setup.sh:24 and :142; BSD sed reads \? as a literal ?.
	it('rejects GNU-only regex escapes in sed', () => {
		expect(found('scripts/setup.sh', `\tsed -n '2,12p' "$0" | sed 's/^# \\?//'`)).toEqual([
			expect.stringMatching(/^1: .*GNU-only/),
		]);
		expect(
			found(
				'scripts/setup.sh',
				`\tsed -n 's/.*version n\\?\\([0-9][0-9]*\\).*/\\1/p' <<<"\${line}"`,
			),
		).toHaveLength(1);
		expect(found('scripts/setup.sh', `\tsed -E 's/^# ?//'`)).toEqual([]);
	});

	it('rejects other GNU or bash 4 forms unless marked', () => {
		expect(
			found('scripts/x.sh', 'sed -i "s/a/b/" file\nreadlink -f x\ndeclare -A map'),
		).toHaveLength(3);
		expect(
			found('scripts/x.sh', '# allow-gnu: runs only in the Linux container\nsed -i "s/a/b/" file'),
		).toEqual([]);
	});

	// 6e0ead37~1:scripts/check-rust-tiers.sh:19-22 hid a cargo tree failure.
	it('rejects || true inside a multi-line command substitution', () => {
		const old = [
			'\thits="$(',
			'\t\tcargo tree --locked -p "$crate" --format \'{p}\' |',
			'\t\t\tawk \'{ print $1 }\' | sort -u | grep -E "$forbidden" || true',
			'\t)"',
		].join('\n');
		expect(found('scripts/check-rust-tiers.sh', old)).toEqual([
			expect.stringMatching(/^3: `\|\| true` hides an error/),
		]);
	});

	it('rejects a silenced single-line capture unless marked with a reason', () => {
		expect(found('scripts/x.sh', 'v="$(tool --version 2>/dev/null)"')).toHaveLength(1);
		expect(
			found('scripts/x.sh', 'v="$(tool --version 2>/dev/null)" # allow-silence: optional probe'),
		).toEqual([]);
		expect(found('scripts/x.sh', 'kill "$pid" 2>/dev/null || true')).toEqual([]);
	});

	// 1c66b9a5:crates/abb-engine/src/remote_source/providers/indexer/connection.rs:518
	it('rejects a permission-bit failure fake in Rust, inline test modules included', () => {
		const line =
			'        std::fs::set_permissions(temp.path(), std::fs::Permissions::from_mode(0o500))';
		expect(
			found('crates/abb-engine/src/remote_source/providers/indexer/connection.rs', line),
		).toEqual([expect.stringMatching(/^1: `from_mode\(0o500\)` fakes an I\/O failure/)]);
		expect(found('crates/abb-engine/src/x.rs', 'permissions.set_mode(0o755);')).toEqual([]);
		expect(
			found(
				'crates/abb-engine/tests/all_tests.rs',
				`// allow-permission-fake: runs as nobody\n${line}`,
			),
		).toEqual([]);
	});

	it('leaves vendored code alone', () => {
		expect(found('vendor/x/build.sh', "sed 's/a\\?//'")).toEqual([]);
	});
});
