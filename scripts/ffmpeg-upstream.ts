/**
 * What changed in FFmpeg master since ABB's pinned commit, in the sources
 * ABB's build can compile.
 *
 * Usage: bun run ffmpeg:upstream
 *
 * Reads the pin from `vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision` and the
 * built components (the lists in that crate's `build.rs`, `CONFIGURE_SELECTED`
 * included). Fetches master into a blobless clone under
 * `target/abb-ffmpeg-upstream/` (reused across runs) and lists the commits
 * after the pin that touch `configure`, libavcodec, libavformat, libavutil or
 * libswresample, leaving out a source file that the pin's and master's
 * Makefiles assign only to components ABB does not build. Then says whether
 * the newest BtbN autobuild was built from master head (`gh api` reads its
 * releases over REST; cloud sessions block GraphQL). Prints a Markdown report;
 * exits non-zero when it could not read the autobuilds.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const SYS_CRATE = 'vendor/ffmpeg-sys-next-9.0.1';
const UPSTREAM_URL = 'https://github.com/FFmpeg/FFmpeg.git';
const BTBN_REPO = 'BtbN/FFmpeg-Builds';
const CACHE_DIRECTORY = 'target/abb-ffmpeg-upstream';

const WATCHED_ROOTS = ['configure', 'libavcodec', 'libavformat', 'libavutil', 'libswresample'];
const COMPONENT_LIBRARIES = ['libavcodec', 'libavformat'];
const REGISTRIES = [
	'libavcodec/allcodecs.c',
	'libavcodec/bitstream_filters.c',
	'libavcodec/hwaccels.h',
	'libavcodec/parsers.c',
	'libavformat/allformats.c',
	'libavformat/protocols.c',
];
const SOURCE_EXTENSIONS = new Set(['.c', '.S', '.asm']);

export interface Component {
	kind: string;
	name: string;
}

export interface Commit {
	hash: string;
	date: string;
	subject: string;
}

export interface Autobuild {
	tag: string;
	asset: string;
	builtFrom: string;
}

export interface Makefile {
	library: string;
	text: string;
}

export type AutobuildRead = { found: Autobuild | null } | { error: string };

export interface Report {
	pin: string;
	head: string;
	commits: Commit[];
	autobuild: AutobuildRead;
}

export function parseComponents(buildRs: string): Component[] {
	const components = [...buildRs.matchAll(/\(\s*"([a-z]+)",\s*&\[([^\]]*)\]/g)].flatMap(
		([, kind, names]) => [...names.matchAll(/"([a-z0-9_]+)"/g)].map(([, name]) => ({ kind, name })),
	);
	if (components.length === 0) throw new Error('build.rs lists no components.');
	return components;
}

/**
 * The condition of every Makefile line that compiles each object:
 * `OBJS-$(CONFIG_<NAME>_<KIND>) += a.o`, its assembly and SIMD variants, and
 * `OBJS += a.o` (condition ''), with `\` continuations. Object paths are
 * relative to the library directory; keys are `<library>/<object without .o>`.
 */
export function objectConditions(makefiles: Makefile[]): Map<string, Set<string>> {
	const conditions = new Map<string, Set<string>>();
	for (const { library, text } of makefiles) {
		for (const line of text.replace(/\\\r?\n/g, ' ').split('\n')) {
			const match = /^(?:[A-Z0-9]+-)?(?:SHLIB|STLIB)?OBJS(?:-\$\(([^)]*)\))?\s*\+?=(.*)$/.exec(
				line,
			);
			if (!match) continue;
			for (const object of match[2].split(/\s+/).filter((word) => word.endsWith('.o'))) {
				const key = `${library}/${object.slice(0, -2)}`;
				const set = conditions.get(key) ?? new Set<string>();
				set.add(match[1] ?? '');
				conditions.set(key, set);
			}
		}
	}
	return conditions;
}

/**
 * The `CONFIG_` name of each component FFmpeg registers (`extern const FFCodec
 * ff_aac_decoder;` is `CONFIG_AAC_DECODER`), as `configure` finds them.
 */
export function registeredComponents(registries: string[]): Set<string> {
	return new Set(
		registries.flatMap((text) =>
			[
				...text.matchAll(
					/\bff_(\w+?_(?:bsf|decoder|demuxer|encoder|hwaccel|muxer|parser|protocol))\b/g,
				),
			].map(([, component]) => `CONFIG_${component.toUpperCase()}`),
		),
	);
}

/**
 * Source files compiled only for registered components ABB does not build.
 * An object stays watched when any line compiles it unconditionally, for a
 * built component, or for a subsystem such as `CONFIG_FRAME_THREAD_ENCODER`.
 */
export function unbuiltSources(
	built: Component[],
	registries: string[],
	makefiles: Makefile[],
	files: Iterable<string>,
): string[] {
	const components = registeredComponents(registries);
	for (const { kind, name } of built) {
		components.delete(`CONFIG_${name.toUpperCase()}_${kind.toUpperCase()}`);
	}
	const unbuilt = new Set<string>();
	for (const [object, conditions] of objectConditions(makefiles)) {
		if ([...conditions].every((when) => components.has(when))) unbuilt.add(object);
	}
	return [...files]
		.filter((file) => {
			const extension = path.extname(file);
			return SOURCE_EXTENSIONS.has(extension) && unbuilt.has(file.slice(0, -extension.length));
		})
		.toSorted();
}

export function watchedPathspecs(unbuilt: string[]): string[] {
	return [...WATCHED_ROOTS, ...unbuilt.map((file) => `:(exclude)${file}`)];
}

export function parseCommits(log: string): Commit[] {
	return log
		.split('\n')
		.filter((line) => line.length > 0)
		.map((line) => {
			const [hash = '', date = '', ...subject] = line.split('\t');
			return { hash, date, subject: subject.join('\t') };
		});
}

export function newestAutobuild(releases: { tag: string; assets: string[] }[]): Autobuild | null {
	for (const { tag, assets } of releases.filter((release) =>
		release.tag.startsWith('autobuild-'),
	)) {
		for (const asset of assets) {
			const match = /^ffmpeg-N-\d+-g([0-9a-f]+)-linux64-gpl\.tar\.xz$/.exec(asset);
			if (match) return { tag, asset, builtFrom: match[1] };
		}
	}
	return null;
}

export function renderReport(report: Report): string {
	const lines = [
		'# FFmpeg upstream watch',
		'',
		`Pinned: \`${report.pin.slice(0, 10)}\`. Master head: \`${report.head.slice(0, 10)}\`.`,
		'',
	];
	if (report.commits.length === 0) {
		lines.push('no relevant upstream changes');
	} else {
		lines.push(
			`${report.commits.length} commit(s) after the pin touch sources ABB's build can compile:`,
			'',
			'| Commit | Date | Subject |',
			'| --- | --- | --- |',
			...report.commits.map(
				(commit) =>
					`| \`${commit.hash.slice(0, 10)}\` | ${commit.date} | ${commit.subject.replaceAll('|', '\\|')} |`,
			),
		);
	}
	lines.push('', '## BtbN autobuild', '');
	if ('error' in report.autobuild) {
		lines.push(`autobuild: could not read (${report.autobuild.error})`);
	} else if (!report.autobuild.found) {
		lines.push('No autobuild release with a linux64-gpl master asset found.');
	} else {
		const { tag, asset, builtFrom } = report.autobuild.found;
		lines.push(
			report.head.startsWith(builtFrom)
				? `\`${tag}\` has \`${asset}\`, built from master head (\`${builtFrom}\`).`
				: `\`${tag}\` has \`${asset}\`, built from \`${builtFrom}\`, not master head.`,
		);
	}
	return lines.join('\n');
}

function run(file: string, args: string[], cwd?: string): string {
	return execFileSync(file, args, {
		cwd,
		encoding: 'utf8',
		maxBuffer: 64 * 1024 * 1024,
		stdio: ['ignore', 'pipe', 'inherit'],
	}).trim();
}

function ensureClone(cache: string): void {
	if (existsSync(path.join(cache, 'HEAD'))) {
		run('git', ['fetch', '--quiet', 'origin', 'master:refs/heads/master'], cache);
		return;
	}
	run('git', [
		'clone',
		'--quiet',
		'--bare',
		'--filter=blob:none',
		'--single-branch',
		'--branch=master',
		UPSTREAM_URL,
		cache,
	]);
}

function readMakefiles(
	cache: string,
	revisions: string[],
): { registries: string[]; makefiles: Makefile[]; files: Set<string> } {
	const registries: string[] = [];
	const makefiles: Makefile[] = [];
	const files = new Set<string>();
	for (const revision of revisions) {
		for (const registry of REGISTRIES) {
			registries.push(run('git', ['show', `${revision}:${registry}`], cache));
		}
		const tree = run(
			'git',
			['ls-tree', '-r', '--name-only', revision, '--', ...COMPONENT_LIBRARIES],
			cache,
		)
			.split('\n')
			.filter((file) => file.length > 0);
		for (const file of tree) files.add(file);
		for (const file of tree.filter((name) => name.endsWith('/Makefile'))) {
			makefiles.push({
				library: file.split('/')[0],
				text: run('git', ['show', `${revision}:${file}`], cache),
			});
		}
	}
	return { registries, makefiles, files };
}

function readAutobuild(): AutobuildRead {
	try {
		const releases = JSON.parse(
			execFileSync(
				'gh',
				[
					'api',
					`repos/${BTBN_REPO}/releases?per_page=10`,
					'--jq',
					'[.[] | {tag: .tag_name, assets: [.assets[].name]}]',
				],
				{ encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] },
			),
		) as { tag: string; assets: string[] }[];
		return { found: newestAutobuild(releases) };
	} catch (error) {
		const stderr = (error as { stderr?: string }).stderr?.trim();
		const reason = stderr || (error instanceof Error ? error.message : String(error));
		return { error: reason.split('\n')[0] };
	}
}

function buildReport(repoRoot: string): Report {
	const pin = readFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), 'utf8').trim();
	const buildRs = readFileSync(path.join(repoRoot, SYS_CRATE, 'build.rs'), 'utf8');
	const cache = path.join(repoRoot, CACHE_DIRECTORY, 'FFmpeg.git');
	ensureClone(cache);

	const { registries, makefiles, files } = readMakefiles(cache, [pin, 'master']);
	const unbuilt = unbuiltSources(parseComponents(buildRs), registries, makefiles, files);
	const head = run('git', ['rev-parse', 'master'], cache);
	const commits = parseCommits(
		run(
			'git',
			['log', '--format=%H%x09%cs%x09%s', `${pin}..master`, '--', ...watchedPathspecs(unbuilt)],
			cache,
		),
	);
	return { pin, head, commits, autobuild: readAutobuild() };
}

if (import.meta.main) {
	try {
		const report = buildReport(path.resolve(import.meta.dirname, '..'));
		console.log(renderReport(report));
		if ('error' in report.autobuild) process.exit(1);
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
