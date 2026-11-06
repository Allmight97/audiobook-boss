import { readFileSync } from 'node:fs';
import path from 'node:path';
import { describe, expect, it } from 'vitest';

const repoRoot = path.resolve(import.meta.dirname, '..');
const IGNORE_LINE = /^\s*"(RUSTSEC-\d{4}-\d{4})",(?:\s*#\s*(\S+))?/;

function ignoredAdvisories(auditToml: string): { id: string; crate: string | undefined }[] {
	return auditToml.split('\n').flatMap((line) => {
		const match = line.match(IGNORE_LINE);
		return match ? [{ id: match[1], crate: match[2] }] : [];
	});
}

function staleIgnores(auditToml: string, cargoLock: string): string[] {
	const locked = new Set([...cargoLock.matchAll(/^name = "([^"]+)"/gm)].map((match) => match[1]));
	return ignoredAdvisories(auditToml)
		.filter(({ crate }) => crate === undefined || !locked.has(crate))
		.map(({ id }) => id);
}

describe('cargo audit ignores', () => {
	it('flags an ignore whose crate Cargo.lock no longer resolves, or that names no crate', () => {
		const lock = 'name = "gtk"\nversion = "0.18.2"\n';
		const toml = [
			'"RUSTSEC-2024-0415", # gtk',
			'"RUSTSEC-2025-0057", # fxhash',
			'"RUSTSEC-2026-0097",',
		];
		expect(staleIgnores(toml.join('\n'), lock)).toEqual(['RUSTSEC-2025-0057', 'RUSTSEC-2026-0097']);
	});

	it('ignores only advisories for crates that Cargo.lock still resolves', () => {
		const toml = readFileSync(path.join(repoRoot, '.cargo/audit.toml'), 'utf8');
		expect(ignoredAdvisories(toml).length).toBeGreaterThan(0);
		expect(staleIgnores(toml, readFileSync(path.join(repoRoot, 'Cargo.lock'), 'utf8'))).toEqual([]);
	});
});
