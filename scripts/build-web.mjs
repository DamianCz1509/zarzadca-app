// Buduje folder dist/ dla aplikacji na komputer: app/zarzadca.html + biblioteki i czcionki lokalnie (działa bez internetu).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const NM = path.join(ROOT, 'node_modules');
const DIST = path.join(ROOT, 'dist');
const LIB = path.join(DIST, 'lib');

fs.rmSync(DIST, { recursive: true, force: true });
fs.mkdirSync(LIB, { recursive: true });

const copy = (from, to) => { const dst = path.join(LIB, to); fs.mkdirSync(path.dirname(dst), { recursive: true }); fs.copyFileSync(path.join(NM, from), dst); };

// OCR (Tesseract, język polski) + podgląd PDF + raporty PDF
copy('tesseract.js/dist/tesseract.min.js', 'tesseract/tesseract.min.js');
copy('tesseract.js/dist/worker.min.js', 'tesseract/worker.min.js');
for (const f of fs.readdirSync(path.join(NM, 'tesseract.js-core')).filter(f => f.endsWith('-lstm.wasm.js'))) copy('tesseract.js-core/' + f, 'tesseract-core/' + f);
copy('@tesseract.js-data/pol/4.0.0_best_int/pol.traineddata.gz', 'tessdata/pol.traineddata.gz');
copy('pdfjs-dist/build/pdf.min.js', 'pdfjs/pdf.min.js');
copy('pdfjs-dist/build/pdf.worker.min.js', 'pdfjs/pdf.worker.min.js');
copy('pdfmake/build/pdfmake.min.js', 'pdfmake/pdfmake.min.js');
copy('pdfmake/build/vfs_fonts.js', 'pdfmake/vfs_fonts.js');

// czcionka Archivo (łacińska + polskie znaki), wagi 400–800
let css = '';
for (const w of [400, 500, 600, 700, 800]) for (const sub of ['latin-ext', 'latin']) {
  const f = `archivo-${sub}-${w}-normal.woff2`;
  copy('@fontsource/archivo/files/' + f, 'fonts/' + f);
  const range = fs.readFileSync(path.join(NM, '@fontsource/archivo', w + '.css'), 'utf8').match(new RegExp(`archivo-${sub}-${w}-normal \\*/[\\s\\S]*?unicode-range: ([^;]+);`));
  css += `@font-face{font-family:'Archivo';font-style:normal;font-display:swap;font-weight:${w};src:url(./${f}) format('woff2');${range ? 'unicode-range:' + range[1] + ';' : ''}}\n`;
}
fs.writeFileSync(path.join(LIB, 'fonts/archivo.css'), css);

// Windows (GitHub) zapisuje pliki z końcami linii CRLF — ujednolicamy, żeby podmiany zawsze trafiały
let html = fs.readFileSync(path.join(ROOT, 'app/zarzadca.html'), 'utf8').replace(/\r\n/g, '\n');
const swap = (from, to) => { if (!html.includes(from)) throw new Error('Nie znaleziono w HTML: ' + from); html = html.split(from).join(to); };
html = html.replace(/<link rel="preconnect" href="https:\/\/fonts\.(googleapis|gstatic)\.com"[^>]*>\n?/g, '');
swap('https://fonts.googleapis.com/css2?family=Archivo:wght@400;500;600;700;800&display=swap', 'lib/fonts/archivo.css');
swap('https://cdn.jsdelivr.net/npm/tesseract.js@5.1.1/dist/tesseract.min.js', 'lib/tesseract/tesseract.min.js');
swap('https://cdn.jsdelivr.net/npm/tesseract.js@5.1.1/dist/worker.min.js', 'lib/tesseract/worker.min.js');
swap('https://cdn.jsdelivr.net/npm/tesseract.js-core@5.1.1', 'lib/tesseract-core');
swap('https://cdn.jsdelivr.net/npm/@tesseract.js-data/pol/4.0.0_best_int', 'lib/tessdata');
swap('https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/pdf.min.js', 'lib/pdfjs/pdf.min.js');
swap('https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/pdf.worker.min.js', 'lib/pdfjs/pdf.worker.min.js');
swap('https://cdn.jsdelivr.net/npm/pdfmake@0.2.10/build/pdfmake.min.js', 'lib/pdfmake/pdfmake.min.js');
swap('https://cdn.jsdelivr.net/npm/pdfmake@0.2.10/build/vfs_fonts.js', 'lib/pdfmake/vfs_fonts.js');
fs.writeFileSync(path.join(DIST, 'index.html'), html);

const size = d => fs.readdirSync(d, { withFileTypes: true }).reduce((a, e) => a + (e.isDirectory() ? size(path.join(d, e.name)) : fs.statSync(path.join(d, e.name)).size), 0);
console.log(`dist/ gotowe: ${(size(DIST) / 1048576).toFixed(1)} MB`);
