/**
 * Fails when an exact pin, override, or release-age exclusion has no
 * `pin: <why>; unpin when <trigger or YYYY-MM-DD>` tag, or its date has passed.
 * TOML files carry the tag as a comment on or above the pin (one tag covers the
 * pins under it up to the next blank or non-pin line). Files that cannot hold
 * comments get a row in the pin table in scripts/AGENTS.md (Dependencies).
 * A table row with no matching pin fails too, so the table cannot go stale.
 *
 * Usage: bun scripts/check-pins.ts
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';

export interface Finding {
	where: string;
	message: string;
}

const TAG = /pin: .+; unpin when (.+)/;
const DATE = /\d{4}-\d{2}-\d{2}/;
const CARGO_PIN = /^\s*([\w-]+)\s*=\s*(?:"=|\{[^}]*version\s*=\s*"=)/;
const COMMENT = /^\s*#/;

export interface PinSources {
	files: Record<string, string>;
	agentsMd: string;
}

interface Pin {
	where: string;
	file: string;
	name: string;
	tag: string | null;
}

function tomlPins(file: string, text: string, pinLine: (line: string) => string | null): Pin[] {
	const pins: Pin[] = [];
	let blockTag: string | null = null;
	for (const line of text.split('\n')) {
		const name = pinLine(line);
		const lineTag = line.match(TAG)?.[0] ?? null;
		if (name) {
			pins.push({ where: `${file}: ${name}`, file, name, tag: lineTag ?? blockTag });
		} else if (COMMENT.test(line)) {
			blockTag = lineTag ?? blockTag;
		} else {
			blockTag = null;
		}
	}
	return pins;
}

function bunfigPins(text: string): Pin[] {
	let inList = false;
	return tomlPins('bunfig.toml', text, (line) => {
		if (/^minimumReleaseAgeExcludes\s*=\s*\[/.test(line)) {
			inList = true;
			return null;
		}
		if (inList && /^\]/.test(line)) inList = false;
		return inList ? (line.match(/^\s*"([^"]+)"/)?.[1] ?? null) : null;
	});
}

function exactNpmVersion(spec: string): boolean {
	return /^(npm:.+@)?\d+\.\d+\.\d+(-[\w.]+)?$/.test(spec);
}

function packageJsonPins(text: string): Pin[] {
	const pkg = JSON.parse(text) as Record<string, Record<string, string> | undefined>;
	const pins: { name: string }[] = [];
	for (const section of ['dependencies', 'devDependencies']) {
		for (const [name, spec] of Object.entries(pkg[section] ?? {})) {
			if (exactNpmVersion(spec)) pins.push({ name });
		}
	}
	for (const name of Object.keys(pkg.overrides ?? {})) pins.push({ name: `${name} (override)` });
	return pins.map(({ name }) => ({
		where: `package.json: ${name}`,
		file: 'package.json',
		name,
		tag: null,
	}));
}

function shellPins(file: string, text: string): Pin[] {
	const names = [
		...text.matchAll(/--from "([\w.-]+)==[^"]+"/g),
		...text.matchAll(/--exclude-newer-package "([\w.-]+)=/g),
	].map((match) => match[1] as string);
	return names.map((name) => ({ where: `${file}: ${name}`, file, name, tag: null }));
}

function tableTags(agentsMd: string): Map<string, string> {
	const tags = new Map<string, string>();
	for (const row of agentsMd.split('\n')) {
		const cells = row.split('|').map((cell) => cell.trim().replaceAll('`', ''));
		const tag = cells[3]?.match(TAG)?.[0];
		if (cells.length >= 5 && cells[1] && cells[2] && tag) tags.set(`${cells[1]}: ${cells[2]}`, tag);
	}
	return tags;
}

function collectPins(files: Record<string, string>): Pin[] {
	const pins: Pin[] = [];
	for (const [file, text] of Object.entries(files)) {
		if (file === 'bunfig.toml') pins.push(...bunfigPins(text));
		else if (file.endsWith('Cargo.toml'))
			pins.push(...tomlPins(file, text, (line) => line.match(CARGO_PIN)?.[1] ?? null));
		else if (file === 'rust-toolchain.toml')
			pins.push(...tomlPins(file, text, (line) => (/^channel\s*=/.test(line) ? 'channel' : null)));
		else if (file === 'package.json') pins.push(...packageJsonPins(text));
		else if (file === '.node-version')
			pins.push({ where: `${file}: node`, file, name: 'node', tag: null });
		else if (file.endsWith('.sh')) pins.push(...shellPins(file, text));
	}
	return pins;
}

export function checkPins({ files, agentsMd }: PinSources, today: string): Finding[] {
	const pins = collectPins(files);
	const table = tableTags(agentsMd);
	const findings: Finding[] = [];
	const matched = new Set<string>();
	for (const pin of pins) {
		const key = `${pin.file}: ${pin.name}`;
		matched.add(key);
		const tag = pin.tag ?? table.get(key) ?? null;
		if (!tag) {
			findings.push({
				where: pin.where,
				message: 'has no `pin: <why>; unpin when <trigger or YYYY-MM-DD>` tag',
			});
			continue;
		}
		const date = tag.match(/unpin when (.+)/)?.[1]?.match(DATE)?.[0];
		if (date && date < today)
			findings.push({
				where: pin.where,
				message: `unpin date ${date} has passed; unpin it or set a new reason`,
			});
	}
	for (const key of table.keys()) {
		if (!matched.has(key))
			findings.push({ where: key, message: 'has a pin table row but no such pin; delete the row' });
	}
	return findings;
}

function trackedPinFiles(): string[] {
	const tracked = execFileSync('git', ['ls-files', '-z'], { encoding: 'utf8' }).split('\0');
	return tracked.filter(
		(file) =>
			/(^|\/)Cargo\.toml$/.test(file) ||
			/^scripts\/[\w.-]+\.sh$/.test(file) ||
			['bunfig.toml', 'rust-toolchain.toml', 'package.json', '.node-version'].includes(file),
	);
}

if (import.meta.main) {
	const files = Object.fromEntries(
		trackedPinFiles().map((file) => [file, readFileSync(file, 'utf8')]),
	);
	const findings = checkPins(
		{ files, agentsMd: readFileSync('scripts/AGENTS.md', 'utf8') },
		new Date().toISOString().slice(0, 10),
	);
	for (const finding of findings) console.error(`${finding.where} ${finding.message}`);
	if (findings.length > 0) process.exit(1);
	console.log('pin tags: ok');
}
