// Użycie: node scripts/set-version.mjs 0.16.0 [owner/repo-z-wydaniami]
// Ustawia numer wersji w aplikacji i adres, z którego aplikacja pobiera aktualizacje.
import fs from 'node:fs';
const [ver, repo] = process.argv.slice(2);
if (!/^\d+\.\d+\.\d+$/.test(ver || '')) { console.error('Podaj wersję w formacie 1.2.3, np. 0.16.0'); process.exit(1); }
const conf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
conf.version = ver;
if (repo) conf.plugins.updater.endpoints = [`https://github.com/${repo}/releases/latest/download/latest.json`];
fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(conf, null, 2) + '\n');
const cargo = fs.readFileSync('src-tauri/Cargo.toml', 'utf8').replace(/^version = ".*"$/m, `version = "${ver}"`);
fs.writeFileSync('src-tauri/Cargo.toml', cargo);
const pkg = JSON.parse(fs.readFileSync('package.json', 'utf8')); pkg.version = ver; fs.writeFileSync('package.json', JSON.stringify(pkg, null, 2) + '\n');
console.log('Wersja', ver, repo ? '· aktualizacje z ' + repo : '');
