import { readFileSync, writeFileSync } from 'node:fs';

const VERSION_RE = /^[0-9]+\.[0-9]+\.[0-9]+(?:-[a-zA-Z0-9.]+)?$/;
const VERSION_SURFACES = [
	'package.json',
	'src-tauri/tauri.conf.json',
	'src-tauri/Cargo.toml',
	'Cargo.lock',
] as const;

type JsonObject = Record<string, unknown>;

const newVersion = process.argv[2] ?? '';

if (!newVersion) {
	console.log(`Current version: ${currentPackageVersion()}`);
	console.log('');
	console.log('Usage: bun scripts/bump-version.ts <new-version>');
	console.log('Example: bun scripts/bump-version.ts 1.0.32');
	process.exit(1);
}

if (!VERSION_RE.test(newVersion)) {
	console.error('Error: Version must be semver format, for example 1.0.32 or 1.0.32-beta.1');
	process.exit(1);
}

updateJsonVersion('package.json', newVersion);
updateJsonVersion('src-tauri/tauri.conf.json', newVersion);
updateCargoPackageVersion('src-tauri/Cargo.toml', 'audiobook-boss', newVersion);
updateCargoLockPackageVersion('Cargo.lock', 'audiobook-boss', newVersion);
verifyVersionSurfaces(newVersion);

console.log(`Bumped version to ${newVersion} in:`);
for (const surface of VERSION_SURFACES) {
	console.log(`  - ${surface}`);
}

function currentPackageVersion(): string {
	const packageJson = readJsonObject('package.json');
	const version = packageJson.version;
	return typeof version === 'string' ? version : 'unknown';
}

function updateJsonVersion(filePath: string, version: string): void {
	const content = readFileSync(filePath, 'utf8');
	const json = parseJsonObject(content, filePath);
	if (typeof json.version !== 'string') {
		throw new Error(`${filePath} does not contain a string version field`);
	}
	writeFileSync(filePath, replaceJsonVersionLine(content, version, filePath));
}

function readJsonObject(filePath: string): JsonObject {
	return parseJsonObject(readFileSync(filePath, 'utf8'), filePath);
}

function parseJsonObject(content: string, filePath: string): JsonObject {
	const parsed = JSON.parse(content) as unknown;
	if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) {
		throw new Error(`${filePath} must contain a JSON object`);
	}
	return parsed as JsonObject;
}

function updateCargoPackageVersion(filePath: string, packageName: string, version: string): void {
	const content = readFileSync(filePath, 'utf8');
	assertTomlPackageVersion(content, filePath, packageName);
	const updated = replacePackageSectionVersion(
		content,
		'[package]',
		packageName,
		version,
		filePath,
	);
	writeFileSync(filePath, updated);
}

function replaceJsonVersionLine(content: string, version: string, filePath: string): string {
	let replacements = 0;
	const updated = content.replace(
		/^(\s*"version"\s*:\s*")[^"]+(".*)$/m,
		(_match, prefix, suffix) => {
			replacements += 1;
			return `${prefix}${version}${suffix}`;
		},
	);
	if (replacements !== 1) {
		throw new Error(`${filePath} must contain exactly one top-level version field`);
	}
	return updated;
}

function updateCargoLockPackageVersion(
	filePath: string,
	packageName: string,
	version: string,
): void {
	const content = readFileSync(filePath, 'utf8');
	assertLockPackageVersion(content, filePath, packageName);
	const blocks = content.split(/(?=^\[\[package\]\]$)/m);
	let updatedCount = 0;
	const updated = blocks
		.map((block) => {
			if (!new RegExp(`^name = "${escapeRegExp(packageName)}"$`, 'm').test(block)) {
				return block;
			}
			updatedCount += 1;
			return replaceVersionLine(block, version, filePath);
		})
		.join('');

	if (updatedCount !== 1) {
		throw new Error(
			`${filePath} expected exactly one ${packageName} package block, found ${updatedCount}`,
		);
	}

	writeFileSync(filePath, updated);
}

function assertTomlPackageVersion(content: string, filePath: string, packageName: string): void {
	const parsed = Bun.TOML.parse(content) as {
		package?: {
			name?: unknown;
			version?: unknown;
		};
	};
	if (parsed.package?.name !== packageName) {
		throw new Error(`${filePath} package.name must be ${packageName}`);
	}
	if (typeof parsed.package.version !== 'string') {
		throw new Error(`${filePath} package.version must be a string`);
	}
}

function assertLockPackageVersion(content: string, filePath: string, packageName: string): void {
	const parsed = Bun.TOML.parse(content) as {
		package?: Array<{
			name?: unknown;
			version?: unknown;
		}>;
	};
	const matches = (parsed.package ?? []).filter((pkg) => pkg.name === packageName);
	if (matches.length !== 1) {
		throw new Error(
			`${filePath} expected exactly one ${packageName} package entry, found ${matches.length}`,
		);
	}
	if (typeof matches[0]?.version !== 'string') {
		throw new Error(`${filePath} ${packageName} version must be a string`);
	}
}

function replacePackageSectionVersion(
	content: string,
	sectionHeader: string,
	packageName: string,
	version: string,
	filePath: string,
): string {
	const lines = content.split('\n');
	let inSection = false;
	let sawExpectedName = false;
	let replaced = false;

	for (let index = 0; index < lines.length; index += 1) {
		const line = lines[index] ?? '';
		if (/^\[[^\]]+\]$/.test(line)) {
			if (inSection && line !== sectionHeader) {
				break;
			}
			inSection = line === sectionHeader;
			continue;
		}
		if (!inSection) {
			continue;
		}
		if (line === `name = "${packageName}"`) {
			sawExpectedName = true;
		}
		if (/^version = "[^"]+"$/.test(line)) {
			lines[index] = `version = "${version}"`;
			replaced = true;
			break;
		}
	}

	if (!sawExpectedName) {
		throw new Error(`${filePath} ${sectionHeader} does not name ${packageName}`);
	}
	if (!replaced) {
		throw new Error(`${filePath} ${sectionHeader} does not contain a version line`);
	}

	return lines.join('\n');
}

function replaceVersionLine(block: string, version: string, filePath: string): string {
	let replacements = 0;
	const updated = block.replace(/^version = "[^"]+"$/m, () => {
		replacements += 1;
		return `version = "${version}"`;
	});
	if (replacements !== 1) {
		throw new Error(`${filePath} package block must contain exactly one version line`);
	}
	return updated;
}

function verifyVersionSurfaces(version: string): void {
	const packageJson = readJsonObject('package.json');
	const tauriConf = readJsonObject('src-tauri/tauri.conf.json');
	const cargoToml = Bun.TOML.parse(readFileSync('src-tauri/Cargo.toml', 'utf8')) as {
		package?: { version?: unknown };
	};
	const cargoLock = Bun.TOML.parse(readFileSync('Cargo.lock', 'utf8')) as {
		package?: Array<{ name?: unknown; version?: unknown }>;
	};
	const cargoLockPackage = (cargoLock.package ?? []).find((pkg) => pkg.name === 'audiobook-boss');

	const surfaces = new Map<string, unknown>([
		['package.json', packageJson.version],
		['src-tauri/tauri.conf.json', tauriConf.version],
		['src-tauri/Cargo.toml', cargoToml.package?.version],
		['Cargo.lock', cargoLockPackage?.version],
	]);

	for (const [surface, found] of surfaces) {
		if (found !== version) {
			throw new Error(`${surface} version mismatch: expected ${version}, found ${String(found)}`);
		}
	}
}

function escapeRegExp(value: string): string {
	return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}
