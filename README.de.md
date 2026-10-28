<div align="center">

# NickelInkSync

**Gleicht den Lesefortschritt seitengeladener E-Books zwischen Kobo, Tolino und KOReader ab.**

[Scope](SCOPE.md) · [Roadmap](ROADMAP.md) · [English](README.md)

</div>

NickelInkSync sorgt dafür, dass du in einem seitengeladenen Buch auf jedem
E-Reader an derselben Stelle weiterliest. Es nutzt einen
[koreader-sync-server](https://github.com/koreader/koreader-sync-server)
(KOSync-Protokoll), sodass Kobo, Tolino und KOReader denselben Fortschritt
teilen.

> [!IMPORTANT]
> **Stand:** frühe Entwicklung. Mit einem per USB angeschlossenen Kobo oder
> Tolino überträgt `nickelinksync push` die Leseposition jedes
> seitengeladenen Buchs zu einem KOSync-Server, `status` stellt die Position
> auf dem Gerät und auf dem Server gegenüber, und `pull` zeigt, was eine
> neuere Position auf dem Server ändern würde; auf einem Tolino schreibt
> `pull --write` sie aufs Gerät (experimentell, vorher mit
> Sicherungskopie). Den Scope und die Datenbank-Befunde, auf denen er beruht,
> findest du in [SCOPE.md](SCOPE.md), die nächsten Schritte in
> [ROADMAP.md](ROADMAP.md) (beides auf Englisch).

## Benutzung

```sh
nickelinksync status --device /Volumes/KOBOeReader
nickelinksync push --device /Volumes/KOBOeReader
nickelinksync pull --device /Volumes/KOBOeReader
```

| Befehl | Was er tut |
|---|---|
| `status` | Listet die seitengeladenen Bücher mit Prüfsumme, gespeicherter Position, KOReader-XPointer und, falls eingerichtet, der Position auf dem Server. |
| `push` | Überträgt die Position jedes Buchs zum Server. Eine Position, die der Server erst nach dem letzten Lesen auf dem Gerät bekommen hat, wird nie überschrieben; außerdem fragt `push` standardmäßig nach, bevor es den Server an eine frühere Stelle im Buch zurücksetzt (siehe `confirm` unten). `--force` überträgt ohne Rückfrage, `--dry-run` zeigt nur, was gesendet würde. |
| `pull` | Zeigt, für welche Bücher der Server eine neuere Position hat, wie das Gerät sie speichern würde und wo es nachfragen würde. Nur mit `--write` wird sie aufs Gerät geschrieben (bisher nur Tolino, experimentell, siehe unten); `--force` übernimmt die Server-Position ohne Rückfrage. |

Alle drei nehmen `--book <Text>` (nur Bücher, deren Titel oder Dateipfad den
Text enthält) und `--config <Datei>` für eine andere Konfigurationsdatei. Das
Gerät wird nur gelesen, nie beschrieben, außer von `pull --write` (und vom
Plugin, siehe unten).

`pull --write` kopiert zuerst `.kobo/KoboReader.sqlite` nach
`~/.local/share/nickelinksync/backups/KoboReader-<UTC-Zeit>.sqlite` (oder
`backups` im Datenordner, siehe unten, oder `$NICKELINKSYNC_BACKUP_DIR`)
und setzt dann die Position aller Bücher in einer Transaktion. Es schreibt
nicht ohne `.kobo/version` (kein eingehängtes Gerät) und nicht, solange
SQLite-Journaldateien neben der Datenbank liegen. Wirf das Gerät aus, bevor
du das Buch öffnest. Rückgängig machen: die Sicherung bei eingehängtem Gerät
zurück nach `.kobo/KoboReader.sqlite` kopieren.

### Buchzuordnung (im Aufbau)

Der Sync-Server erkennt ein Buch an einer Prüfsumme der Datei. Ein kepub auf
dem Kobo, ein anders umgewandeltes EPUB auf dem Tolino oder ein KFX auf dem
Kindle hat eine andere Prüfsumme als das Original-EPUB, das KOReader öffnet.
Eine Zuordnung verbindet so ein Buch mit seinem Original-EPUB:

```sh
nickelinksync map suggest --device /Volumes/KOBOeReader --library ~/Calibre-Bibliothek
nickelinksync map add --device /Volumes/KOBOeReader --book "Heat 2" --epub ~/Books/heat-2.epub
nickelinksync map add --device /Volumes/Kindle --book heat-2
nickelinksync map list
nickelinksync map remove 1a2b3c4d
```

| Befehl | Was er tut |
|---|---|
| `map suggest` | Für jedes Buch auf dem Gerät (oder die mit `--book`/`--content-id` gewählten): schon zugeordnet, dieselbe Datei wie eine in der Bibliothek (keine Zuordnung nötig) oder bis zu drei Kandidaten aus der Bibliothek mit Bewertung und dem `map add`-Befehl für den besten. Speichert nichts. |
| `map add` | Ordnet ein Buch auf dem Gerät (`--book <Text>` oder `--content-id`) dem mit `--epub` angegebenen Original-EPUB zu und sagt, wie viele Kapitel der beiden Dateien übereinstimmen. Ohne `--epub` zeigt es die Vorschläge aus der Bibliothek und fragt, welcher es sein soll (nur im Terminal). Eine Datei, die mit dem EPUB identisch ist, braucht keine Zuordnung und wird nicht gespeichert. Wird dasselbe Buch erneut zugeordnet, ersetzt das die alte Zuordnung. |
| `map list` | Zeigt alle Zuordnungen und prüft ihr EPUB: Fehlt es oder hat es sich seitdem geändert, wird die Zuordnung als kaputt gemeldet. |
| `map remove` | Löscht die Zuordnung mit dieser Prüfsumme bzw. Kindle-`content_id` (oder einem eindeutigen Anfang davon). |

Ein Buch auf Kobo oder Tolino wird an der Prüfsumme seiner Datei erkannt;
Verschieben oder Umbenennen auf dem Gerät behält die Zuordnung. Ein
Kindle-Buch wird an seiner `content_id` erkannt. Weder Gerät noch EPUB
werden verändert. **Noch nicht:** `status`, `push` und `pull` nutzen die
Zuordnungen bisher nicht.

**Bibliothek.** Die Original-EPUBs kommen aus einem Ordner, angegeben mit
`--library` oder als `library` in der Konfigurationsdatei. Liegt dort eine
Calibre-Bibliothek (`metadata.db`), kommen Titel, Autoren und EPUB-Pfade
aus Calibres Datenbank, nur lesend als unveränderliche Datei geöffnet;
sonst wird jedes `*.epub` unterhalb des Ordners gelesen (nicht
`*.kepub.epub`), Titel und Autoren aus seiner OPF. NickelInkSync schreibt
nie in die Bibliothek.

**Bewertung.** Vorschläge vergleichen den Titel (gemeinsame Wörter mit dem
Titel oder Dateinamen der EPUB; bei einem Kindle-Buch ist der Titel der
Name der gesendeten Datei), die Autoren und bei den besten Kandidaten die
Kapitel (Spine-Einträge nach Pfad bzw. KFX-Abschnitte gegen
Spine-Einträge): Titel × 0,7 + Autor × 0,3, dann 0,8 × das + 0,2 ×
Kapitel. Akzente und Umlaute werden vereinfacht, Zahlen zählen mit, „Band
1“ und „Band 2“ unterscheiden sich also. Kandidaten unter 50 % werden nicht
gezeigt, unter 80 % ist der beste als „uncertain“ markiert.

Die Zuordnungen liegen in `mappings.json` im Datenordner
`~/.local/share/nickelinksync/` (oder `$XDG_DATA_HOME/nickelinksync`, oder
`$NICKELINKSYNC_DATA_DIR`), den sich CLI und der kommende Desktop-Client
teilen.

## Konfiguration

`push` und `pull` brauchen eine Konfigurationsdatei, `status` nutzt sie, wenn
es sie gibt. Standardort: `~/.config/nickelinksync/config.ini` (oder
`$XDG_CONFIG_HOME/nickelinksync/config.ini`, oder der Pfad in
`$NICKELINKSYNC_CONFIG`). Das Format steht im [englischen README](README.md#configuration):
`server`, `user`, `password` (oder `key` = MD5 des Passworts), optional
`device`, `device_id`, `confirm`, `notify` (nur für das Tolino-Plugin,
siehe unten) und `library` (Ordner mit den Original-EPUBs für die
Buchzuordnung; dafür genügt eine Datei, die nur `library` enthält).

`confirm` legt fest, wann `push` und `pull` „Gerät oder Server?“ fragen,
statt die neuere Position gewinnen zu lassen:

| Wert | Fragt |
|---|---|
| `never` | nie: die neuere Position gewinnt |
| `backwards` (Standard) | wenn die Position, die überschrieben würde, weiter hinten im Buch liegt als die neue |
| `always` | immer, wenn sich die beiden Positionen unterscheiden |

„Weiter hinten“ entscheidet die Stelle im Buch (Kapitel, dann die Stelle im
Text des Kapitels, Prozent nur, wenn das nicht entscheidet), nicht die
Uhrzeit: Nickel vermerkt ein Buch schon beim Öffnen als gelesen,
ein auf dem E-Reader nur geöffnetes Buch wirkt also neuer als die Seite, die
du in KOReader erreicht hast. Im Terminal antwortest du mit `d` (Gerät) oder
`s` (Server), Enter überspringt das Buch. Ohne Terminal wird nichts
überschrieben und das Buch gemeldet. `--force` überschreibt ohne Rückfrage.

Das Passwort steht nur in dieser Datei, nie in der Befehlszeile. Halte die
Datei privat (`chmod 600`); NickelInkSync warnt, wenn andere sie lesen
können.

## Plugin für den Tolino (experimentell)

Auf einem Tolino mit Firmware 5.x gleicht ein NickelHook-Plugin ohne
Rechner ab, per WLAN: Beim Öffnen eines seitengeladenen Buchs holt es eine
neuere Position vom Server, und das Buch öffnet dort; beim Verlassen des
Buchs sendet es deine. Es
ruft dasselbe Programm `nickelinksync` auf dem Gerät auf und folgt derselben
Regel `confirm`; fragen kann es noch nicht, eine Position, die eine Antwort
bräuchte, bleibt deshalb unverändert und steht im Log. Auf einem Kobo bleibt
das Plugin vorerst inaktiv.

1. `NickelInkSync-tolino` aus dem letzten CI-Lauf holen (oder `just
   package`, siehe [Bauen](#bauen)). Darin liegen `Kobo.tgz` und ein Ordner
   `.adds/NickelInkSync/` mit dem Programm und `config.ini.example`.
2. Tolino per USB verbinden. Den Ordner `NickelInkSync` in den versteckten
   Ordner `.adds/` kopieren und dort `config.ini.example` als `config.ini`
   mit deinem Konto speichern (Format wie [oben](#konfiguration)).
3. `Kobo.tgz` in den versteckten Ordner `.kobo/` kopieren und auswerfen.
   Der Tolino installiert das Plugin und startet neu.

Das Log steht in `.adds/NickelInkSync/NickelInkSync.log`. Das Plugin schaltet
das WLAN selbst ein, wenn es es braucht; „Jetzt synchronisieren“ des Tolino
ist nicht nötig (das ist die Tolino-Cloud). Beim Öffnen eines Buchs wartet es
höchstens etwa 10 Sekunden auf das WLAN (ohne Dialog) und 15 Sekunden auf
den Server. Damit das Buch an der Server-Position öffnet, schreibt das Plugin
sie kurz vor dem Öffnen in die Zeile des Buchs in der Gerätedatenbank,
während Nickel läuft; einmal am Tag sichert es die Datenbank nach
`.adds/NickelInkSync/backups/` (die zwei neuesten bleiben). Ohne WLAN öffnet
das Buch unverändert, und beim Verlassen wird dann nichts gesendet, damit eine
ältere Tolino-Position nie eine neuere auf dem Server überschreibt. Beim
Verlassen wartet es nie: Der Push folgt, sobald das WLAN da ist, nach 15
Sekunden mit dem WLAN-Dialog des Tolino.

Öffnet Nickel ein Buch nicht an der Position, die das Plugin geschrieben
hat (beobachtet beim ersten Öffnen nach dem Start des Tolino), vermerkt
das Plugin es in `.adds/NickelInkSync/not-in-line.tsv`. Ein solches Buch
wird nie gesendet, weder vom Plugin noch mit `push` am Rechner (außer mit
`--force`), weil die Stelle, die Nickel gespeichert hat, nicht deine ist;
der nächste `pull`, beim Öffnen oder per USB, übernimmt wieder die
Server-Position, auch wenn „zuletzt gelesen“ auf dem Tolino jetzt neuer
aussieht.

Jedes Ergebnis zeigt das Plugin als kurze Meldung unten auf dem Bildschirm
(vorerst auf Deutsch), etwa „Fortschritt vom Server übernommen“ mit Prozent
und dem Gerät, von dem die Position kam, oder „Fortschritt konnte nicht vom
Server gelesen werden“ mit dem Grund (keine Verbindung, Zeitüberschreitung,
Anmeldung abgelehnt). `notify` in `config.ini` legt fest, welche:

| Wert | Zeigt |
|---|---|
| `all` (Standard) | jedes Ergebnis, auch „Fortschritt ist aktuell“, wenn nichts zu übernehmen oder zu senden war (etwa wenn du ein Buch auf der Seite verlässt, auf der es mit der Server-Position geöffnet wurde) |
| `errors` | nur, was schiefging oder unterblieb: Server nicht erreicht, Position nicht übernommen, nicht gesendet |
| `none` | nichts; im Log steht weiterhin alles |

**Deinstallieren:** eine leere Datei `.adds/NickelInkSync/uninstall`
anlegen, auswerfen und den Tolino aus- und wieder einschalten (die Datei
wird nur beim Start geprüft).

## Geräte und Bücher

- Kobo mit Firmware 4.x sowie Tolino Shine 5, Shine Color und Vision Color mit
  Firmware 5.x
- Seitengeladene kepub- und EPUB-Bücher ohne DRM
- Vorerst muss auf jedem Gerät **dieselbe Datei** liegen. Der Sync-Server
  erkennt Bücher an einer Prüfsumme der Datei, ein kepub auf dem einen und das
  ursprüngliche EPUB auf dem anderen Gerät gelten also als zwei verschiedene
  Bücher. Die manuelle Zuordnung ist im Aufbau (siehe
  [Buchzuordnung](#buchzuordnung-im-aufbau)).

## Vorgeschichte

In diesem Repository lag früher `inksync`, ein Export von Markierungen und
Lesezeit vom Kobo nach Obsidian. Er wurde mit dem Scope-Wechsel entfernt; die
letzte Version steht in der Git-Historie vor diesem Wechsel.

## Bauen

Benötigt eine stabile [Rust](https://rustup.rs)-Toolchain.

```sh
cargo build --release
```

Das Programm liegt danach unter `target/release/nickelinksync`. HTTPS läuft
über `rustls` mit eingebauten Stammzertifikaten, OpenSSL wird nicht gebraucht,
auch nicht auf dem Gerät.

Für das Gerät (Kobo und Tolino, 32-Bit-ARM) baut man ein statisch gelinktes
Programm mit [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild):

```sh
rustup target add armv7-unknown-linux-musleabihf
pip install cargo-zigbuild   # installiert auch Zig
just build-device
```

Es liegt dann unter `target/armv7-unknown-linux-musleabihf/release/nickelinksync`.
`just check-device` ruft zusätzlich `--version` in einem ARM-Container auf
(braucht Docker). Jeder CI-Lauf lädt dieselbe Datei als Artefakt
`nickelinksync-armv7-musleabihf` hoch.

Das Plugin (`plugin/`, NickelHook als Submodul) wird für Qt 6.5.2 im
Toolchain-Image `ghcr.io/dkuester/nickelstorygraph-toolchain` gebaut
(Docker, amd64): `just plugin` erzeugt `plugin/Kobo.tgz`, `just package`
legt es mit dem Geräteprogramm in `dist/NickelInkSync/` zusammen. Mit
`--recurse-submodules` klonen oder `git submodule update --init` ausführen.

## Support

| | |
|---|---|
| Fehler und Ideen | [Issues](https://github.com/dkuester/NickelInkSync/issues). Bitte Gerätemodell und Firmware-Version angeben. |
| Mitwirken | Pull-Requests gegen `main`. |

## Danksagung

NickelInkSync baut auf der Arbeit dieser Projekte und ihrer Maintainer auf.
Die Tabelle mit Lizenzen und Links steht im [englischen README](README.md#credits).

- **Code, den es nutzt:** [NickelHook](https://github.com/pgaskin/NickelHook)
  von Patrick Gaskin (MIT), die Grundlage des Plugins, und dessen Qt6-Port im
  [Branch `qt6-support`](https://github.com/nicoverbruggen/NickelHook/tree/qt6-support)
  von Nico Verbruggen (MIT), das Submodul des Plugins; dazu die Rust-Crates
  aus `Cargo.toml` (clap, md-5, quick-xml, rusqlite mit SQLite, serde, ureq
  mit rustls, zip).
- **Ideen und Dokumentation, kein Code** (nachgebaut nach dem beschriebenen
  Verhalten): [KOReader](https://github.com/koreader/koreader) (Prüfsumme,
  XPointer, Fortschritts-Sync) und
  [koreader-sync-server](https://github.com/koreader/koreader-sync-server)
  (KOSync-Protokoll), beide AGPL-3.0;
  [BookOrbit](https://github.com/bookorbit/bookorbit) (AGPL-3.0, Zuordnung
  über die KOReader-Prüfsumme); [Kobalt](https://github.com/nicoverbruggen/kobalt)
  von Nico Verbruggen (MIT, Nickels Buch-Cache und Schreiben über Nickel
  selbst); die [Amazon-Ion-1.0-Spezifikation](https://amazon-ion.github.io/ion-docs/)
  (KFX, Kindle-Machbarkeit).

## Lizenz

[MIT](LICENSE)
