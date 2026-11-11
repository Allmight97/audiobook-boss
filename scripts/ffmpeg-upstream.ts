/**
 * What changed in FFmpeg master since ABB's pinned commit, limited to the
 * source files of the components ABB builds.
 *
 * Usage: bun run ffmpeg:upstream
 *
 * Reads the pin from `vendor/ffmpeg-sys-next-9.0.1/ffmpeg-revision` and the
 * enabled components from `ABB_COMPONENTS` in that crate's `build.rs`.
 * Fetches master into a blobless clone under `target/abb-ffmpeg-upstream/`
 * (reused across runs), lists the commits after the pin that touch those
 * files, and says whether the newest BtbN autobuild was built from master
 * head (`gh api` reads its releases over REST; cloud sessions block GraphQL). Prints a Markdown report and exits 0.
 */
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';

const SYS_CRATE = 'vendor/ffmpeg-sys-next-9.0.1';
const UPSTREAM_URL = 'https://github.com/FFmpeg/FFmpeg.git';
const BTBN_REPO = 'BtbN/FFmpeg-Builds';
const CACHE_DIRECTORY = 'target/abb-ffmpeg-upstream';

const ARCH_MAKEFILES = ['libavcodec/aarch64/Makefile', 'libavcodec/x86/Makefile'];

const ALWAYS_WATCHED = [
	'configure',
	'libavcodec/aacenc*',
	'libavcodec/aaccoder*',
	'libavcodec/aacpsy*',
	'libavcodec/decode.c',
	'libavcodec/encode.c',
	'libavcodec/mpeg4audio*',
	'libavcodec/adts_header*',
	'libavcodec/mpegaudio*',
	'libavformat/mov.c',
	'libavformat/movenc.c',
	'libavformat/isom*',
	'libavformat/riff*',
	'libavformat/id3v2*',
	'libavformat/demux.c',
	'libavformat/mux.c',
	'libavformat/utils.c',
	'libavformat/aviobuf.c',
	'libavutil/tx*',
	'libavutil/*/tx*',
	'libavutil/float_dsp*',
	'libavutil/*/float_dsp*',
	'libavutil/aes*',
	'libavutil/*/aes*',
	'libavutil/encryption_info*',
	'libswresample',
];

const KIND_LIBRARY: Record<string, 'libavcodec' | 'libavformat'> = {
	decoder: 'libavcodec',
	encoder: 'libavcodec',
	parser: 'libavcodec',
	bsf: 'libavcodec',
	demuxer: 'libavformat',
	muxer: 'libavformat',
	protocol: 'libavformat',
};

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

export interface Report {
	pin: string;
	head: string;
	commits: Commit[];
	autobuild: Autobuild | null;
	autobuildMatchesHead: boolean;
}

function constBody(buildRs: string, name: string): string {
	const start = buildRs.indexOf(`const ${name}`);
	if (start === -1) throw new Error(`build.rs has no ${name}.`);
	const end = buildRs.indexOf('];', start);
	return buildRs.slice(start, end);
}

function quotedNames(text: string): string[] {
	return [...text.matchAll(/"([a-z0-9_]+)"/g)].map((match) => match[1]);
}

export function parseComponents(buildRs: string): Component[] {
	const body = constBody(buildRs, 'ABB_COMPONENTS');
	const components: Component[] = [];
	for (const group of body.matchAll(/\(\s*"([a-z]+)",\s*&\[([^\]]*)\]/g)) {
		const kind = group[1];
		for (const name of quotedNames(group[2])) components.push({ kind, name });
	}
	for (const extra of [
		{ kind: 'encoder', name: 'libopus' },
		{ kind: 'decoder', name: 'aac_at' },
		{ kind: 'encoder', name: 'aac_at' },
	]) {
		if (new RegExp(`\\("${extra.kind}",\\s*&\\[[^\\]]*"${extra.name}"`).test(buildRs)) {
			components.push(extra);
		}
	}
	return components;
}

export function makefileIncludes(makefile: string): string[] {
	return [...makefile.matchAll(/^include \$\(SRC_PATH\)\/([^\s$]+\/Makefile)\s*$/gm)].map(
		(match) => match[1],
	);
}

/**
 * Pathspecs for the sources FFmpeg's Makefiles build for each component:
 * `OBJS-$(CONFIG_<NAME>_<KIND>) += a.o b.o`, with `\` continuations, and the
 * assembly and SIMD variants of that line. Object paths are relative to the
 * library directory. Each object `a.o` becomes `a*`, which also takes its
 * headers and the `a_template.c` files it includes.
 */
export function sourcesOf(
	components: Component[],
	makefiles: { library: string; text: string }[],
): string[] {
	const wanted = new Map<string, string>();
	for (const { kind, name } of components) {
		const library = KIND_LIBRARY[kind];
		if (library) wanted.set(`CONFIG_${name.toUpperCase()}_${kind.toUpperCase()}`, library);
	}
	const sources = new Set<string>();
	for (const { library, text } of makefiles) {
		const joined = text.replace(/\\\r?\n/g, ' ');
		for (const line of joined.split('\n')) {
			const match = /^(?:[A-Z0-9]+-)?OBJS-\$\((CONFIG_\w+)\)\s*\+?=\s*(.*)$/.exec(line);
			if (!match || wanted.get(match[1]) !== library) continue;
			for (const object of match[2].split(/\s+/)) {
				if (object.endsWith('.o')) sources.add(`${library}/${object.slice(0, -2)}*`);
			}
		}
	}
	return [...sources].toSorted();
}

export function patchedFiles(patch: string): string[] {
	return [...patch.matchAll(/^\+\+\+ b\/(\S+)$/gm)].map((match) => match[1]);
}

export function watchedPatterns(
	buildRs: string,
	patch: string,
	makefiles: { library: string; text: string }[],
): string[] {
	return [
		...new Set([
			...sourcesOf(parseComponents(buildRs), makefiles),
			...ALWAYS_WATCHED,
			...patchedFiles(patch),
		]),
	].toSorted();
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

export function newestAutobuild(
	tags: string[],
	assetsOf: (tag: string) => string[],
): Autobuild | null {
	for (const tag of tags.filter((name) => name.startsWith('autobuild-'))) {
		for (const asset of assetsOf(tag)) {
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
			`${report.commits.length} commit(s) after the pin touch the sources of the enabled components:`,
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
	if (!report.autobuild) {
		lines.push('No autobuild release with a linux64-gpl master asset found.');
	} else {
		const { tag, asset, builtFrom } = report.autobuild;
		lines.push(
			report.autobuildMatchesHead
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

function readMakefiles(cache: string): { library: string; text: string }[] {
	const show = (file: string) => run('git', ['show', `master:${file}`], cache);
	const result: { library: string; text: string }[] = [];
	for (const library of ['libavcodec', 'libavformat']) {
		const main = show(`${library}/Makefile`);
		result.push({ library, text: main });
		const includes = makefileIncludes(main).filter((include) => include.startsWith(`${library}/`));
		const arch = ARCH_MAKEFILES.filter((file) => file.startsWith(`${library}/`));
		for (const include of [...includes, ...arch]) {
			result.push({ library, text: show(include) });
		}
	}
	return result;
}

function buildReport(repoRoot: string): Report {
	const pin = readFileSync(path.join(repoRoot, SYS_CRATE, 'ffmpeg-revision'), 'utf8').trim();
	const buildRs = readFileSync(path.join(repoRoot, SYS_CRATE, 'build.rs'), 'utf8');
	const patch = readFileSync(
		path.join(repoRoot, SYS_CRATE, 'patches/mov-chapter-start.patch'),
		'utf8',
	);
	const cache = path.join(repoRoot, CACHE_DIRECTORY, 'FFmpeg.git');
	ensureClone(cache);

	const watched = watchedPatterns(buildRs, patch, readMakefiles(cache));
	const head = run('git', ['rev-parse', 'master'], cache);
	const commits = parseCommits(
		run('git', ['log', '--format=%H%x09%cs%x09%s', `${pin}..master`, '--', ...watched], cache),
	);

	const releases = JSON.parse(
		run('gh', [
			'api',
			`repos/${BTBN_REPO}/releases?per_page=10`,
			'--jq',
			'[.[] | {tag: .tag_name, assets: [.assets[].name]}]',
		]),
	) as { tag: string; assets: string[] }[];
	const assetsByTag = new Map(releases.map((release) => [release.tag, release.assets]));
	const autobuild = newestAutobuild(
		releases.map((release) => release.tag),
		(tag) => assetsByTag.get(tag) ?? [],
	);
	return {
		pin,
		head,
		commits,
		autobuild,
		autobuildMatchesHead: autobuild !== null && head.startsWith(autobuild.builtFrom),
	};
}

if (import.meta.main) {
	try {
		console.log(renderReport(buildReport(path.resolve(import.meta.dirname, '..'))));
	} catch (error) {
		console.error(`Error: ${error instanceof Error ? error.message : String(error)}`);
		process.exit(1);
	}
}
