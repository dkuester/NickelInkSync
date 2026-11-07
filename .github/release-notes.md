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
- **Device package (experimental):** first **back up the device's database** (`.kobo/KoboReader.sqlite`, copy it away over USB). Then, still over USB: copy the `.adds/NickelInkSync` folder into the hidden `.adds/` folder, save `config.ini.example` there as `config.ini` with your account, run `nickelinksync device prepare --device <the mounted device>` (or "Prepare device" in the desktop client), copy `Kobo.tgz` into the hidden `.kobo/` folder and eject. The device installs the plugin and restarts. `Kobo.tgz` is for a Kobo on firmware 6.0 and a Tolino on 5.18; a **Kobo on firmware 4.x** takes `kobo-4.x/KoboRoot.tgz` instead (each firmware installs only its own file). To remove the plugin, create an empty file `.adds/NickelInkSync/uninstall` and restart the device.

### Supported devices (experimental)

The plugin is experimental. Tested, and the only ones supported: **Kobo Clara BW** on firmware 6.0.279225, **Kobo Libra 2** on 4.38.23852 and **Tolino Shine Color** on 5.18.279364. On other firmware nobody has checked it. Kindle support is experimental too (see below).

### Good to know

- **Conflicts have no dialog in this version.** If a book moved on both the device and the server since they last agreed, and the server's place lies further back, the plugin leaves the book as it is, fetches nothing and sends nothing; a message ("Konflikt, nicht geladen") and a log line say so. Settle it on the computer, in the desktop client or with `nickelinksync pull` / `push`. The dialog follows in 0.1.1, together with "sync now" and mapping on the device (neither is in 0.1.0).
- **The device has to be "prepared" on the computer** (`device prepare` or "Prepare device"; again after mapping or adding books). Without it the plugin only fetches, never sends, and says "Gerät nicht vorbereitet".
- **Back up the device database before the first use.** NickelInkSync also makes a backup of its own before it writes (once a day on the device); `nickelinksync restore` or "Restore a backup" puts one back.
- **While Kobalt is installed, the plugin stays inactive** ("NickelInkSync ist aus"). Remove Kobalt and it works again after the next start.
- **NickelStorygraph:** use a current version. The fix that lets it chain with NickelInkSync is merged there (its PR #16); an older version can bypass NickelInkSync's opening hook, so a book opens without fetching the server's position.
- A kepub syncs only once it is mapped to its original EPUB (the file KOReader opens), under that EPUB's checksum, never under its own. The plugin on the device knows the mappings that `device prepare` put there; on a device that was never prepared it sends nothing, on the Tolino too. The same holds for a Kindle book.
- Kindle support is experimental: reading a copy of a Kindle's books and sending positions work, nothing goes back to the Kindle.

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
- **Gerätepaket (experimentell):** zuerst die **Datenbank des Geräts sichern** (`.kobo/KoboReader.sqlite` per USB wegkopieren). Dann, weiter per USB: den Ordner `.adds/NickelInkSync` in den versteckten Ordner `.adds/` kopieren, dort `config.ini.example` als `config.ini` mit deinem Konto speichern, `nickelinksync device prepare --device <das eingehängte Gerät>` ausführen (oder „Gerät vorbereiten“ im Desktop-Client), `Kobo.tgz` in den versteckten Ordner `.kobo/` kopieren und auswerfen. Das Gerät installiert das Plugin und startet neu. `Kobo.tgz` ist für einen Kobo mit Firmware 6.0 und einen Tolino mit 5.18; ein **Kobo mit Firmware 4.x** bekommt stattdessen `kobo-4.x/KoboRoot.tgz` (jede Firmware installiert nur ihre eigene Datei). Zum Entfernen des Plugins eine leere Datei `.adds/NickelInkSync/uninstall` anlegen und das Gerät neu starten.

### Unterstützte Geräte (experimentell)

Das Plugin ist experimentell. Getestet und allein unterstützt: **Kobo Clara BW** mit Firmware 6.0.279225, **Kobo Libra 2** mit 4.38.23852 und **Tolino Shine Color** mit 5.18.279364. Auf anderer Firmware hat es niemand geprüft. Auch die Kindle-Unterstützung ist experimentell (siehe unten).

### Gut zu wissen

- **Konflikte haben in dieser Version keinen Dialog.** Hat sich ein Buch seit dem letzten Abgleich auf Gerät und Server bewegt und liegt die Stelle des Servers weiter hinten, lässt das Plugin das Buch unverändert, holt nichts und sendet nichts; eine Meldung („Konflikt, nicht geladen“) und ein Logeintrag sagen das. Aufgelöst wird am Rechner, im Desktop-Client oder mit `nickelinksync pull` / `push`. Der Dialog kommt in 0.1.1, zusammen mit „Jetzt synchronisieren“ und dem Zuordnen am Gerät (beides gibt es in 0.1.0 nicht).
- **Das Gerät muss am Rechner „vorbereitet“ sein** (`device prepare` oder „Gerät vorbereiten“; wieder nach dem Zuordnen oder Hinzufügen von Büchern). Ohne das holt das Plugin nur, sendet nie und sagt „Gerät nicht vorbereitet“.
- **Vor dem ersten Einsatz die Datenbank des Geräts sichern.** NickelInkSync legt vor jedem Schreiben außerdem eine eigene Sicherung an (auf dem Gerät einmal am Tag); `nickelinksync restore` oder „Sicherung zurückspielen“ spielt sie zurück.
- **Solange Kobalt installiert ist, bleibt das Plugin inaktiv** („NickelInkSync ist aus“). Kobalt entfernen, und es arbeitet nach dem nächsten Start wieder.
- **NickelStorygraph:** eine aktuelle Version nehmen. Die Korrektur, mit der es sich mit NickelInkSync verkettet, ist dort gemergt (PR #16); eine ältere Version kann den Öffnen-Hook von NickelInkSync umgehen, dann öffnet ein Buch, ohne die Position vom Server zu holen.
- Ein kepub wird erst synchronisiert, wenn es seiner Original-EPUB (der Datei, die KOReader öffnet) zugeordnet ist, unter deren Prüfsumme, nie unter seiner eigenen. Das Plugin auf dem Gerät kennt die Zuordnungen, die `device prepare` dort ablegt; auf einem nie vorbereiteten Gerät sendet es nichts, auch auf dem Tolino. Für ein Kindle-Buch gilt dasselbe.
- Die Kindle-Unterstützung ist experimentell: Eine Kopie der Kindle-Bücher lesen und Positionen senden geht, zurück auf den Kindle geht nichts.
