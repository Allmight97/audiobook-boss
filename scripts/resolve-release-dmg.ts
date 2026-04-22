import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';

interface TauriConfig {
	productName?: string;
}

function usage(): string {
	return [
		'Usage: bun scripts/resolve-release-dmg.ts (--version <x.y.z> | --tag <vX.Y.Z>)',
		'',
		'Options:',
		'  --version <x.y.z>      Release version to resolve in target/release/bundle/dmg.',
		'  --tag <vX.Y.Z>         Release tag; leading "v" is stripped before lookup.',
		'  --help                 Show this help.',
	].join('\n');
}

function isSemver(version: string): boolean {
	return /^[0-9]+\.[0-9]+\.[0-9]+(?:-[a-zA-Z0-9.]+)?$/.test(version);
}

function readProductName(repoRoot: string): string {
	const configPath = path.join(repoRoot, 'src-tauri', 'tauri.conf.json');
	const config = JSON.parse(readFileSync(configPath, 'utf8')) as TauriConfig;
	if (!config.productName) {
		throw new Error(`Missing productName in ${configPath}`);
	}
	return config.productName;
}

function matchesReleaseDmg(name: string, productName: string, version: string): boolean {
	return name.startsWith(`${productName}_${version}_`) && name.endsWith('.dmg');
}

export function resolveReleaseDmgArtifact(repoRoot: string, version: string): string {
	if (!isSemver(version)) {
		throw new Error(`Release version must be semver (e.g., 1.0.12). Received '${version}'.`);
	}

	const productName = readProductName(repoRoot);
	const bundleDir = path.join(repoRoot, 'target', 'release', 'bundle', 'dmg');
	if (!existsSync(bundleDir)) {
		throw new Error(`DMG bundle directory not found at ${bundleDir}`);
	}

	const matches = readdirSync(bundleDir)
		.filter((entry) => matchesReleaseDmg(entry, productName, version))
		.sort();

	if (matches.length === 0) {
		throw new Error(`No DMG matched ${path.join(bundleDir, `${productName}_${version}_*.dmg`)}`);
	}

	if (matches.length !== 1) {
		const resolved = matches.map((entry) => path.join(bundleDir, entry)).join('\n');
		throw new Error(
			`Expected exactly one DMG for release ${version}, found ${matches.length}.\n${resolved}`,
		);
	}

	return path.join(bundleDir, matches[0]);
}

function parseVersionArg(args: string[]): string {
	let version = '';
	let tag = '';

	for (let index = 0; index < args.length; index += 1) {
		const token = args[index];
		if (token === '--version') {
			version = args[index + 1] ?? '';
			index += 1;
			continue;
		}
		if (token === '--tag') {
			tag = args[index + 1] ?? '';
			index += 1;
			continue;
		}
		if (token === '--help' || token === '-h') {
			console.log(usage());
			process.exit(0);
		}
		throw new Error(`Unknown option '${token}'.`);
	}

	if (version && tag) {
		throw new Error('Pass either --version or --tag, not both.');
	}

	if (tag) {
		version = tag.startsWith('v') ? tag.slice(1) : tag;
	}

	if (!version) {
		throw new Error(`Pass --version or --tag.\n\n${usage()}`);
	}

	return version;
}

function main(): void {
	try {
		const version = parseVersionArg(process.argv.slice(2));
		const repoRoot = path.resolve(import.meta.dir, '..');
		console.log(resolveReleaseDmgArtifact(repoRoot, version));
	} catch (error) {
		const message = error instanceof Error ? error.message : String(error);
		console.error(`Error: ${message}`);
		process.exit(1);
	}
}

if (import.meta.main) {
	main();
}
