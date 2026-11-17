import { describe, expect, it } from 'vitest';
import {
	ALLOWLIST,
	commentFindings,
	isPolicyFile,
	keptComments,
	stripComments,
} from './comment-policy';

function messages(file: string, source: string): string[] {
	return commentFindings(file, source).map((finding) => finding.message);
}

describe('comment policy', () => {
	it('gives every allowlist entry a reason', () => {
		expect(ALLOWLIST.map((entry) => entry.id)).toEqual([
			'license',
			'lint-directive',
			'shell-directive',
			'permission-fake',
			'safety',
			'upstream',
			'pin',
			'ratchet',
			'rustdoc-test',
		]);
		for (const entry of ALLOWLIST) {
			expect(entry.reason.trim().length).toBeGreaterThan(20);
		}
	});

	it('skips vendored, generated, frozen settings, and frozen fixtures', () => {
		expect(isPolicyFile('crates/abb-engine/src/lib.rs')).toBe(true);
		expect(isPolicyFile('vendor/opus/src/lib.rs')).toBe(false);
		expect(isPolicyFile('src/lib/generated/tauri.ts')).toBe(false);
		expect(isPolicyFile('crates/abb-engine/src/app_settings/saved_settings/settings.toml')).toBe(
			false,
		);
		expect(isPolicyFile('scripts/fixtures/ci-64ec2240-parent.yml')).toBe(false);
		expect(isPolicyFile('README.md')).toBe(false);
	});

	it('fails a new comment and names the fix and the root cause', () => {
		const message = messages('src/app/x.ts', 'const value = 1;\n// retry it later\n');
		expect(message).toHaveLength(1);
		expect(message[0]).toContain('Fix: delete it');
		expect(message[0]).toContain('Root cause:');
		expect(message[0]).toContain('scripts/comment-policy.ts');
	});

	it('keeps a lint directive only when it carries a reason', () => {
		const kept =
			'// oxlint-disable-next-line typescript/no-explicit-any -- the fixture is untyped\n';
		expect(messages('src/app/x.ts', kept)).toEqual([]);
		expect(
			messages('src/app/x.ts', '// oxlint-disable-next-line typescript/no-explicit-any\n'),
		).toEqual([expect.stringContaining('directive with no reason')]);
		expect(messages('vite.config.ts', '// @ts-expect-error process is a nodejs global\n')).toEqual(
			[],
		);
	});

	it('keeps a SAFETY comment and its continuation, and rejects a bare one', () => {
		const source =
			'    // SAFETY: the handle is open.\n    // Drop closes it.\n    unsafe { free(handle) }\n';
		expect(stripComments('crates/x/src/lib.rs', source)).toBe(source);
		expect(
			messages('crates/x/src/lib.rs', '    // SAFETY:\n    unsafe { free(p) }\n'),
		).toHaveLength(1);
	});

	it('keeps an upstream bug link with its paragraph and drops the prose around a pin', () => {
		const cargo = [
			'# The lib name collides on Windows.',
			'# See https://github.com/rust-lang/cargo/issues/8519',
			'specta = "=2.0.0"',
			'# Bump the family together.',
			'# pin: prerelease family; unpin when specta 2.0.0 is stable',
			'',
		].join('\n');
		expect(stripComments('Cargo.toml', cargo)).toBe(
			[
				'# The lib name collides on Windows.',
				'# See https://github.com/rust-lang/cargo/issues/8519',
				'specta = "=2.0.0"',
				'# pin: prerelease family; unpin when specta 2.0.0 is stable',
				'',
			].join('\n'),
		);
	});

	it('keeps a rustdoc test and drops the prose around it', () => {
		const source = [
			'//! Hosts can read this.',
			'//!',
			'//! ```',
			'//! use crate::Engine;',
			'//! ```',
			'pub struct Engine;',
			'',
		].join('\n');
		expect(stripComments('crates/x/src/lib.rs', source)).toBe(
			['//! ```', '//! use crate::Engine;', '//! ```', 'pub struct Engine;', ''].join('\n'),
		);
		expect(keptComments('crates/x/src/lib.rs', source).map((kept) => kept.id)).toEqual([
			'rustdoc-test',
			'rustdoc-test',
			'rustdoc-test',
		]);
	});

	it('does not treat a slash pair inside a regular expression as a comment', () => {
		const source =
			"return source.replace(/\\/\\*[\\s\\S]*?\\*\\//g, ' ').replace(/\\/\\/.*$/gm, ' ');\n";
		expect(stripComments('src/x.ts', source)).toBe(source);
	});

	it('does not treat comment markers in strings, raw strings, or a heredoc as comments', () => {
		const rust =
			'const TEXT: &str = r#" // not a comment "#;\nlet url = "https://example.com/a#b";\n';
		expect(stripComments('crates/x/src/lib.rs', rust)).toBe(rust);
		const shell = ["cat <<'USAGE'", '  # this is help text', 'USAGE', 'echo hi # goes', ''].join(
			'\n',
		);
		expect(stripComments('scripts/x.sh', shell)).toBe(
			["cat <<'USAGE'", '  # this is help text', 'USAGE', 'echo hi', ''].join('\n'),
		);
	});

	it('keeps shell and permission directives and drops a JSX comment with its braces', () => {
		expect(
			messages('scripts/x.sh', 'v="$(tool 2>/dev/null)" # allow-silence: missing is the answer\n'),
		).toEqual([]);
		const tsx = [
			'export const View = () => (',
			'\t<div>',
			'\t\t{/* retry later */}',
			'\t\t<span />',
			'\t</div>',
			');',
			'',
		].join('\n');
		expect(stripComments('src/ui/View.tsx', tsx)).toBe(
			['export const View = () => (', '\t<div>', '\t\t<span />', '\t</div>', ');', ''].join('\n'),
		);
		const kept =
			'\t\t{/* oxlint-disable-next-line jsx-a11y/no-static-element-interactions -- the backdrop closes on click. */}\n';
		expect(messages('src/ui/View.tsx', kept)).toEqual([]);
	});

	it('leaves a workflow prompt block alone and strips a YAML comment', () => {
		const yaml = [
			'      - run: |',
			'          echo hi # stays in the script? no, run blocks are shell',
			'      - name: Review',
			'        with:',
			'          prompt: |',
			'            # leave this instruction',
			'          model: fable # pinned',
			'',
		].join('\n');
		expect(stripComments('.github/workflows/ci.yml', yaml)).toBe(
			[
				'      - run: |',
				'          echo hi',
				'      - name: Review',
				'        with:',
				'          prompt: |',
				'            # leave this instruction',
				'          model: fable',
				'',
			].join('\n'),
		);
	});

	it('keeps a license header', () => {
		const header =
			'/*\n * Copyright (c) 2026 Ada Lovelace\n * SPDX-License-Identifier: MIT\n */\nexport const n = 1;\n';
		expect(stripComments('scripts/x.ts', header)).toBe(header);
	});
});
