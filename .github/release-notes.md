<!-- Template for the release workflow (.github/workflows/release.yml):
     @VERSION@ becomes the version. The draft can be edited before
     publishing; add the highlights at the top. -->

## NickelInkSync @VERSION@

Syncs the reading position of sideloaded books between Kobo, Tolino, Kindle (experimental) and KOReader through a KOSync server.

### Downloads

| File | For |
|---|---|
| `NickelInkSync-@VERSION@-macos.dmg` | Desktop client, macOS 11 or later (Apple silicon and Intel) |
| `nickelinksync-cli-@VERSION@-macos.zip` | Command line, macOS |
| `NickelInkSync-@VERSION@-windows-x64.zip` | Desktop client, Windows 10 or later (x64) |
| `nickelinksync-cli-@VERSION@-windows-x64.zip` | Command line, Windows |
| `NickelInkSync-@VERSION@-device.zip` | **Experimental:** the program and plugin for the Tolino and Kobo |
| `SHA256SUMS` | Checksums of all files (`shasum -a 256 -c SHA256SUMS`) |

### Installing

- **macOS:** open the `.dmg` and drag NickelInkSync into Applications. The app is not notarized, so macOS blocks the first start: open it once, then System Settings → Privacy & Security → "Open Anyway". For the command line: unpack, then `xattr -d com.apple.quarantine nickelinksync`.
- **Windows:** unpack the ZIP anywhere and start `NickelInkSync.exe`; keep `kepubify.exe` next to it. The program is not code-signed, so SmartScreen warns: "More info" → "Run anyway".
- **Device package (experimental):** connect the device over USB. Copy the `.adds/NickelInkSync` folder into the hidden `.adds/` folder, save `config.ini.example` there as `config.ini` with your account, run `nickelinksync device prepare --device <the mounted device>` (or "Prepare device" in the desktop client), copy `Kobo.tgz` into the hidden `.kobo/` folder and eject. The device installs the plugin and restarts. `Kobo.tgz` is for a Kobo on firmware 6.x and a Tolino on 5.x; a **Kobo on firmware 4.x** takes `kobo-4.x/KoboRoot.tgz` instead (each firmware installs only its own file). On 4.x the plugin loads but does not sync yet. **While Kobalt is installed, the plugin stays inactive.**

### Good to know

- A kepub syncs only once it is mapped to its original EPUB (the file KOReader opens), under that EPUB's checksum, never under its own. The plugin on the device knows the mappings that `device prepare` put there; on a device that was never prepared it sends nothing, on the Tolino too. The same holds for a Kindle book.
- Kindle support is experimental: reading a copy of a Kindle's books and sending positions work, nothing goes back to the Kindle.
- Before writing to a device database, NickelInkSync makes a backup; "Restore a backup" (desktop client) or `nickelinksync restore` puts one back.

---

## NickelInkSync @VERSION@ (deutsch)

Gleicht die Leseposition seitengeladener Bücher zwischen Kobo, Tolino, Kindle (experimentell) und KOReader über einen KOSync-Server ab.

### Downloads

| Datei | Für |
|---|---|
| `NickelInkSync-@VERSION@-macos.dmg` | Desktop-Client, macOS ab 11 (Apple Silicon und Intel) |
| `nickelinksync-cli-@VERSION@-macos.zip` | Kommandozeile, macOS |
| `NickelInkSync-@VERSION@-windows-x64.zip` | Desktop-Client, Windows ab 10 (x64) |
| `nickelinksync-cli-@VERSION@-windows-x64.zip` | Kommandozeile, Windows |
| `NickelInkSync-@VERSION@-device.zip` | **Experimentell:** Programm und Plugin für Tolino und Kobo |
| `SHA256SUMS` | Prüfsummen aller Dateien (`shasum -a 256 -c SHA256SUMS`) |

### Installation

- **macOS:** das `.dmg` öffnen und NickelInkSync in „Programme“ ziehen. Die App ist nicht notarisiert, daher blockiert macOS den ersten Start: einmal öffnen, dann Systemeinstellungen → Datenschutz & Sicherheit → „Dennoch öffnen“. Für die Kommandozeile: entpacken, dann `xattr -d com.apple.quarantine nickelinksync`.
- **Windows:** das ZIP irgendwohin entpacken und `NickelInkSync.exe` starten; `kepubify.exe` bleibt daneben liegen. Das Programm ist nicht signiert, daher warnt SmartScreen: „Weitere Informationen“ → „Trotzdem ausführen“.
- **Gerätepaket (experimentell):** das Gerät per USB verbinden. Den Ordner `.adds/NickelInkSync` in den versteckten Ordner `.adds/` kopieren, dort `config.ini.example` als `config.ini` mit deinem Konto speichern, `nickelinksync device prepare --device <das eingehängte Gerät>` ausführen (oder „Gerät vorbereiten“ im Desktop-Client), `Kobo.tgz` in den versteckten Ordner `.kobo/` kopieren und auswerfen. Das Gerät installiert das Plugin und startet neu. `Kobo.tgz` ist für einen Kobo mit Firmware 6.x und einen Tolino mit 5.x; ein **Kobo mit Firmware 4.x** bekommt stattdessen `kobo-4.x/KoboRoot.tgz` (jede Firmware installiert nur ihre eigene Datei). Auf 4.x lädt das Plugin, synchronisiert aber noch nicht. **Solange Kobalt installiert ist, bleibt das Plugin inaktiv.**

### Gut zu wissen

- Ein kepub wird erst synchronisiert, wenn es seiner Original-EPUB (der Datei, die KOReader öffnet) zugeordnet ist, unter deren Prüfsumme, nie unter seiner eigenen. Das Plugin auf dem Gerät kennt die Zuordnungen, die `device prepare` dort ablegt; auf einem nie vorbereiteten Gerät sendet es nichts, auch auf dem Tolino. Für ein Kindle-Buch gilt dasselbe.
- Die Kindle-Unterstützung ist experimentell: Eine Kopie der Kindle-Bücher lesen und Positionen senden geht, zurück auf den Kindle geht nichts.
- Bevor NickelInkSync in eine Gerätedatenbank schreibt, legt es eine Sicherung an; „Sicherung zurückspielen“ (Desktop-Client) oder `nickelinksync restore` spielt sie zurück.
