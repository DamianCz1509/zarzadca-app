[README.md](https://github.com/user-attachments/files/33986664/README.md)
# zarzadca-app# Zarządca: desktop app for Windows and Mac

This is the same app as the browser version, packaged as a normal program for Windows and macOS (built with Tauri). Both versions use the same data folder, so a Windows PC and a Mac can work on the same OneDrive folder at the same time. The desktop version adds:

- **A data folder of your choice**, ideally in OneDrive. The app keeps everything there:
  - `dane.json`: all the data;
  - `Skany\`: contract scans, plus the text read from them;
  - `kopie\`: one backup copy per day, the last 30 days.
- **Several people working at once.** The folder is shared through OneDrive and changes reach the others within a few seconds:
  - if two people edit the same record, the newer change is kept;
  - if OneDrive creates a conflict copy (`dane-COMPUTER.json`), the app merges it automatically;
  - the top bar shows who else is working right now.
- **One-click updates.** When you publish a new version, the app offers "Aktualizuj do X.Y" (update to X.Y). It saves the data, installs the update and restarts. Data and scans are not touched.
- **Works offline.** OCR, the PDF viewer and fonts are built into the installer.

Installers:
- **Windows:** `Zarzadca_X.Y.Z_x64-setup.exe`. It installs for the current user only and needs no administrator rights.
- **Mac:** `Zarzadca_X.Y.Z_universal.dmg`. One file works on both Apple Silicon (M1–M4) and Intel Macs, macOS 10.15 or newer.

---

## One-time setup

Follow the step-by-step guide **INSTRUKCJA.html**, which comes with these files. It takes about 20 minutes, plus about 40 minutes of waiting for the first build.

---

## Installing on a computer
1. Run `Zarzadca_…_x64-setup.exe`.
   - Until you buy a code-signing certificate, Windows will show "System Windows ochronił ten komputer" (Windows protected your PC). Click **Więcej informacji → Uruchom mimo to** (More info → Run anyway).
2. On first launch, the app suggests the folder `OneDrive\Zarządca`.
   - Accept it, or choose another folder.
   - Then type your full name, which your colleagues will see.
3. Moving data from the browser version:
   - in the browser: **Ustawienia → Kopia danych → Pobierz kopię**;
   - in the desktop app: **Ustawienia → Folder danych i kopia → Wgraj kopię**.
   - If your scans were in a folder on disk, copy that folder's contents into `Zarządca\Skany`.

## Installing on a Mac
1. Install **OneDrive for Mac** from the App Store and sign in. Its folder appears in Finder under *Lokalizacje* (Locations).
2. Open `Zarzadca_…_universal.dmg` and drag **Zarzadca** onto **Applications**.
3. First launch: without an Apple certificate (see below), macOS shows "nie można sprawdzić dewelopera" (the developer can't be verified).
   - **macOS 15 and newer:** click **Gotowe** (Done), then open **Ustawienia systemowe → Prywatność i ochrona**. At the bottom, click **Otwórz mimo to** (Open anyway) and enter your password.
   - **Older macOS:** right-click the app in Applications → **Otwórz** (Open) → **Otwórz**.
   - This is needed only once. Updates later install without it.
4. The app suggests the folder `OneDrive…/Zarządca` automatically. On a Mac, the shortcuts are ⌘K for search and ⌘C / ⌘V as usual.

**Apple certificate (optional, recommended before selling to clients):** with an Apple Developer Program membership (99 USD a year), the Mac app can be signed and notarised, and the "can't verify developer" warning disappears. It needs a few extra secrets in GitHub, which I can add to the workflow when you have the account.

## Several people on the same data (OneDrive)
1. **First person:** in OneDrive, right-click the `Zarządca` folder → **Udostępnij** (Share), and give the others edit access.
2. **Each colleague:**
   - opens the link and clicks **Dodaj skrót do folderu Moje pliki** (Add shortcut to My files). The folder then appears in their own OneDrive.
   - installs the app (Windows or Mac) and, on first launch, uses **Wybierz inny folder…** to point to that shared folder. The app will say "znaleziono dane" (data found).
3. **Recommended in OneDrive:** right-click the folder → **Zawsze zachowuj na tym urządzeniu** (Always keep on this device), so scans open without delay.

> A shared folder suits a small team well. For many people working all day on the same data, the next step is a dedicated server with logins.

---

## Publishing an update (over the air)
1. Replace `app/zarzadca.html` in the repository with the new version: open the `app` folder → **Add file → Upload files** → drop the new `zarzadca.html` → **Commit changes**.
2. Go to **Actions → Wydanie aplikacji → Run workflow**.
   - Enter a **higher** version number, e.g. `0.17.0`.
   - Add a short description of what's new.
3. After about 30–40 minutes, every installed app (Windows and Mac) finds the update within 6 hours, or straight away on next start or via **Ustawienia → Sprawdź aktualizacje**. It then shows **Aktualizuj do 0.17.0** at the top of the screen.

## Private code (optional)
1. Keep the code repository private.
2. Create a second, **public** repository just for installers, e.g. `zarzadca-wydania`, with a README.
3. In the private repository, set:
   - **Settings → Secrets and variables → Actions → Variables:** `RELEASES_REPO` = `your-login/zarzadca-wydania`;
   - **Secrets:** `RELEASES_TOKEN` = a token from **GitHub → Settings → Developer settings → Fine-grained tokens**, with Contents: Read and write access to `zarzadca-wydania`.
4. The workflow then publishes installers and updates in the public repository.

---

## For developers
- `npm ci`, then `npx tauri build`. This builds `dist/` itself via `scripts/build-web.mjs`.
- `scripts/set-version.mjs 0.17.0 owner/repo` sets the version number and the update address.
- `npx tauri build --target universal-apple-darwin --bundles app,dmg` builds the Mac version (on a Mac).
- `src-tauri/src/lib.rs` handles file operations limited to the data folder, atomic saving, folder selection, "Pokaż w Eksploratorze/Finderze", OneDrive detection (Windows and Mac) and the updater.
- `app/zarzadca.html` is the same single file as the browser version. It recognises the desktop mode by `window.__TAURI__`.
