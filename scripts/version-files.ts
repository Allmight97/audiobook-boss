import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import path from 'node:path';

import { VERSION_SURFACES } from './bump-version';

export interface VersionSurface {
	file: string;
	version: string | null;
}

export interface VersionFinding {
	file: string;
	line: number;
	message: string;
}

export interface VersionReleaseInput {
	versions: VersionSurface[];
	changelog: string;
	tags: string[];
}

const VERSION = /^[0-9]+\.[0-9]+\.[0-9]+$/;

const HELP = `Usage: bun scripts/version-files.ts [--print | --notes]

Exits 0 when every version file agrees. When that version differs from the
latest vX.Y.Z tag, CHANGELOG.md must also have a non-empty '## [x.y.z]' section.
The date on that heading is not checked. --print writes the agreed version.
--notes writes that changelog section.`;

export function latestStableTag(tags: string[]): string | null {
	const stable = tags.filter((tag) => /^v\d+\.\d+\.\d+$/.test(tag));
	stable.sort((left, right) => compareSemver(right.slice(1), left.slice(1)));
	return stable[0] ?? null;
}

function changelogReleaseNotes(changelog: string, version: string): string | null {
	const section = changelogSection(changelog, version);
	if (!section || section.problem || section.body.length === 0) return null;
	return `${section.body}\n`;
}

export function versionReleaseFindings(input: VersionReleaseInput): VersionFinding[] {
	const versions = versionFindings(input.versions);
	if (versions.length > 0) return versions;
	const version = input.versions[0]?.version;
	if (!version) return versions;
	const latest = latestStableTag(input.tags);
	if (latest === `v${version}`) return [];
	const section = changelogSection(input.changelog, version);
	const latestLabel = latest ?? 'none';
	if (!section) {
		return [
			{
				file: 'CHANGELOG.md',
				line: 1,
				message: `Version ${version} differs from the latest tag ${latestLabel}, and CHANGELOG.md has no non-empty '## [${version}]' section. Fix: rename '## [Unreleased]' to '## [${version}] - YYYY-MM-DD' and write the user-facing notes under that heading.`,
			},
		];
	}
	if (!section.problem) return [];
	return [
		{
			file: 'CHANGELOG.md',
			line: section.line,
			message: section.problem.replace('latest tag', `latest tag ${latestLabel}`),
		},
	];
}

function readVersionRelease(repoRoot: string): VersionReleaseInput {
	return {
		versions: readVersions(repoRoot),
		changelog: readFileSync(path.join(repoRoot, 'CHANGELOG.md'), 'utf8'),
		tags: localTags(repoRoot),
	};
}

export function repoVersionReleaseFindings(repoRoot: string): VersionFinding[] {
	try {
		return versionReleaseFindings(readVersionRelease(repoRoot));
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		return [
			{
				file: 'package.json',
				line: 1,
				message: `Version files could not be read (${message}). Fix: set every version file to the same x.y.z with \`bun scripts/bump-version.ts <x.y.z>\`.`,
			},
		];
	}
}

function versionFindings(versions: VersionSurface[]): VersionFinding[] {
	const described = versions
		.map((surface) => `${surface.file} is ${surface.version ?? 'missing'}`)
		.join(', ');
	const present = versions.filter((surface) => surface.version !== null);
	const distinct = [...new Set(present.map((surface) => surface.version))];
	const fix =
		'Fix: set every version file to the same x.y.z with `bun scripts/bump-version.ts <x.y.z>`.';
	if (present.length !== versions.length || distinct.length !== 1) {
		return [
			{
				file: versions[0]?.file ?? 'package.json',
				line: 1,
				message: `Version files disagree: ${described}. ${fix}`,
			},
		];
	}
	const version = distinct[0];
	if (version && VERSION.test(version)) return [];
	return [
		{
			file: versions[0]?.file ?? 'package.json',
			line: 1,
			message: `Version ${version ?? 'missing'} is not x.y.z (${described}). ${fix}`,
		},
	];
}

interface Section {
	line: number;
	body: string;
	problem: string | null;
}

function changelogSection(changelog: string, version: string): Section | null {
	const heading = `## [${version}]`;
	const lines = changelog.split('\n');
	const start = lines.findIndex((line) => line === heading || line.startsWith(`${heading} `));
	if (start === -1) return null;
	const actual = lines[start] ?? '';
	const dated = new RegExp(`^## \\[${escapeRegExp(version)}\\](?: - \\d{4}-\\d{2}-\\d{2})?$`);
	if (!dated.test(actual)) {
		return {
			line: start + 1,
			body: '',
			problem: `CHANGELOG.md heading '${actual}' must be '${heading}' or '${heading} - YYYY-MM-DD'. Fix: retitle that section; the date does not have to be today.`,
		};
	}
	let end = lines.findIndex((line, index) => index > start && line.startsWith('## '));
	if (end === -1) end = lines.length;
	const body = lines
		.slice(start + 1, end)
		.join('\n')
		.trim();
	if (body.length === 0) {
		return {
			line: start + 1,
			body: '',
			problem: `Version ${version} differs from the latest tag, and CHANGELOG.md section '${heading}' is empty. Fix: write the user-facing notes under that heading.`,
		};
	}
	return { line: start + 1, body, problem: null };
}

function compareSemver(left: string, right: string): number {
	const a = left.split('.').map(Number);
	const b = right.split('.').map(Number);
	for (let index = 0; index < 3; index += 1) {
		const difference = (a[index] ?? 0) - (b[index] ?? 0);
		if (difference !== 0) return difference;
	}
	return 0;
}

function readVersions(repoRoot: string): VersionSurface[] {
	return VERSION_SURFACES.map((file) => ({ file, version: readSurface(repoRoot, file) }));
}

function readSurface(repoRoot: string, file: string): string | null {
	const content = readFileSync(path.join(repoRoot, file), 'utf8');
	if (file.endsWith('.json')) {
		const parsed = JSON.parse(content) as { version?: unknown };
		return typeof parsed.version === 'string' ? parsed.version : null;
	}
	if (file.endsWith('.toml')) {
		const parsed = Bun.TOML.parse(content) as { package?: { version?: unknown } };
		return typeof parsed.package?.version === 'string' ? parsed.package.version : null;
	}
	const parsed = Bun.TOML.parse(content) as {
		package?: Array<{ name?: unknown; version?: unknown }>;
	};
	const match = (parsed.package ?? []).find((pkg) => pkg.name === 'audiobook-boss');
	return typeof match?.version === 'string' ? match.version : null;
}

function localTags(repoRoot: string): string[] {
	return execFileSync('git', ['tag', '--list', 'v*'], { cwd: repoRoot, encoding: 'utf8' })
		.split('\n')
		.map((tag) => tag.trim())
		.filter((tag) => tag.length > 0);
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function main(): number {
	if (process.argv.includes('--help') || process.argv.includes('-h')) {
		console.log(HELP);
		return 0;
	}
	const repoRoot = path.resolve(import.meta.dirname, '..');
	const findings = repoVersionReleaseFindings(repoRoot);
	for (const finding of findings) {
		console.error(`${finding.file}:${finding.line}: ${finding.message}`);
	}
	if (findings.length > 0) return 1;
	if (process.argv.includes('--notes')) {
		const input = readVersionRelease(repoRoot);
		const version = input.versions[0]?.version ?? '';
		const notes = version ? changelogReleaseNotes(input.changelog, version) : null;
		if (!notes) {
			console.error(
				`CHANGELOG.md has no non-empty '## [${version}]' section. Fix: rename '## [Unreleased]' to '## [${version}] - YYYY-MM-DD' and write the user-facing notes under that heading.`,
			);
			return 1;
		}
		process.stdout.write(notes);
		return 0;
	}
	if (process.argv.includes('--print')) {
		const version = readVersionRelease(repoRoot).versions[0]?.version;
		if (!version) return 1;
		console.log(version);
	}
	return 0;
}

if (import.meta.main) {
	process.exit(main());
}
