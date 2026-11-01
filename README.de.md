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
| `sync --pending` | Sendet die Bücher, deren Push auf dem Gerät gescheitert ist (kein Netz, Server-Fehler), wie sie in `.adds/NickelInkSync/state.json` vorgemerkt sind, sofern sie noch gesendet werden sollen (siehe [Zuletzt abgeglichene Position](#zuletzt-abgeglichene-position)). |

Alle drei nehmen `--book <Text>` (nur Bücher, deren Titel oder Dateipfad den
Text enthält), `--config <Datei>` für eine andere Konfigurationsdatei und
`--library <Ordner>` für die Bibliothek mit den Originalen (siehe
[Buchzuordnung](#buchzuordnung-im-aufbau)); sie entscheidet mit, welche
Bücher synchronisiert werden. Das
Gerät wird nur gelesen, nie beschrieben, außer von `pull --write` (und vom
Plugin, siehe unten); `push`, `pull --write` und `sync --pending` vermerken
außerdem die mit dem Server abgeglichene Position in
`.adds/NickelInkSync/state.json`.

### Zuletzt abgeglichene Position

Für jedes Buch merkt sich das Gerät die Position, auf die es sich zuletzt
mit dem Server geeinigt hat (nach einem `push`, einem `pull --write` oder
wenn beide schon dieselbe hatten), in `.adds/NickelInkSync/state.json`,
gemeinsam für Plugin und CLI per USB. Damit schauen `push` und `pull`, was
sich seitdem bewegt hat:

- nur der Server: Seine Position gewinnt, auch wenn „zuletzt gelesen“ auf
  dem Gerät neuer ist (Nickel setzt es schon beim Öffnen eines Buchs);
- nur das Gerät: Seine Position gewinnt, auch wenn sie weiter vorn im Buch
  liegt;
- beide: Die neuere gewinnt, und `confirm` entscheidet, wann gefragt wird,
  wie ohne Basis (siehe [Konfiguration](#konfiguration)).

`status` zeigt die Basis (`base`) und was sich seitdem bewegt hat
(`since`). Ein Buch, das zum ersten Mal abgeglichen wird, hat noch keine
Basis und folgt der Regel ohne. Scheitert ein Push auf dem Gerät
(`push --live`), weil kein Netz da ist oder der Server einen Fehler meldet,
wird das Buch vorgemerkt; `sync --pending` fragt für jedes vorgemerkte Buch
den Server erneut und sendet es, wenn es noch gesendet werden soll
(`status` zeigt `pending`). `confirm = always` fragt weiterhin bei jedem
Unterschied.

`pull --write` kopiert zuerst `.kobo/KoboReader.sqlite` nach
`~/.local/share/nickelinksync/backups/KoboReader-<UTC-Zeit>.sqlite` (oder
`backups` im Datenordner, siehe unten, oder `$NICKELINKSYNC_BACKUP_DIR`)
und setzt dann die Position aller Bücher in einer Transaktion. Es schreibt
nicht ohne `.kobo/version` (kein eingehängtes Gerät) und nicht, solange ein
SQLite-Rollback-Journal (`-journal`) neben der Datenbank liegt. Die Dateien
`-wal` und `-shm`, die der Tolino 5.18.279364 auf der USB-Partition liegen
lässt, arbeitet es in die Datenbank ein (die Sicherung enthält ihren Inhalt
dann mit). Wirf das Gerät aus, bevor
du das Buch öffnest. Auf dem Kobo 6.0 und dem Tolino 5.18.279364 öffnet
Nickel ein Buch aus seinem Zwischenspeicher fertig gezeichneter Seiten und,
wenn keine gespeicherte Seite die geschriebene Stelle enthält, auf der
ersten gespeicherten Seite dieses Kapitels. Deshalb verwirft das Schreiben
auch die gespeicherten Seiten dieses Kapitels (nur dieses Kapitels und nur,
wenn keine die Stelle enthält); es meldet das, und dann musst du das Gerät
neu starten, nicht nur auswerfen, weil Nickel diese Seiten im Speicher
hält.

Rückgängig machen: `nickelinksync restore --device /Volumes/KOBOeReader`
listet die Sicherungen dieses Geräts, die neueste zuerst, und `--backup
PFAD` spielt eine zurück: erst eine Sicherung des jetzigen Stands, dann die
alte über SQLite (so mischt sich keine liegen gebliebene `-wal` hinein);
danach das Gerät auswerfen und neu starten. Neben jeder Sicherung liegt
eine `.device`-Datei, die ihr Gerät über eine Prüfsumme der Seriennummer
nennt; die Sicherung eines anderen Geräts, eine aus einer anderen
Datenbankversion (seitdem ein Firmware-Update) und, solange Kobalt
installiert ist, jede Sicherung wird abgelehnt. Ältere Sicherungen stehen
als „Gerät unbekannt“ in der Liste. Der Desktop-Client macht dasselbe mit
„Sicherung zurückspielen …“.

### Buchzuordnung (im Aufbau)

Der Sync-Server erkennt ein Buch an einer Prüfsumme der Datei. Ein kepub auf
dem Kobo, ein anders umgewandeltes EPUB auf dem Tolino oder ein KFX auf dem
Kindle hat eine andere Prüfsumme als das Original-EPUB, das KOReader öffnet.
Eine Zuordnung verbindet so ein Buch mit seinem Original-EPUB:

```sh
nickelinksync map suggest --device /Volumes/KOBOeReader --library ~/Calibre-Bibliothek
nickelinksync map add --device /Volumes/KOBOeReader --book "Heat 2" --epub ~/Books/heat-2.epub
nickelinksync map add --device ~/Kindle-Kopie --book heat-2
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
werden verändert.

**Synchronisieren über die Zuordnung.** `status`, `push` und `pull`
entscheiden für jedes Buch, unter welcher Prüfsumme es synchronisiert wird
(Zeile `sync as`):

| Buch | Synchronisiert |
|---|---|
| zugeordnet, EPUB unverändert | unter der Prüfsumme der Original-EPUB, Position übersetzt (siehe unten) |
| zugeordnet, EPUB fehlt oder geändert | gar nicht, bis es neu zugeordnet ist |
| dieselbe Datei wie in der Bibliothek, oder die Original-EPUB einer Zuordnung | unter der eigenen Prüfsumme |
| ein kepub (`.kepub.epub` oder MimeType `application/x-kobo-epub+zip`) ohne Zuordnung | gar nicht („not mapped“), mit oder ohne Bibliothek: KOReader kennt das Buch nur unter seiner Original-EPUB, ein kepub wird also nie unter seiner eigenen Prüfsumme synchronisiert |
| nichts davon, Bibliothek eingerichtet | gar nicht („not mapped“; `map suggest` hilft) |
| keine Bibliothek eingerichtet (EPUB) | unter der eigenen Prüfsumme, wie bisher |

Die Position wird zwischen Gerätedatei und Original über das Kapitel (den
Pfad der Datei; von Calibre geteilte Kapitel `ch1_split_000.xhtml`,
`_split_001` … zählen als `ch1.xhtml`) und den sichtbaren Text des Kapitels
übersetzt (ohne Leerraum und weiche Trennstriche). Wo etwas nicht passt,
sagt die Ausgabe es: *found by its text* (der Text weicht ab, die Stelle
wurde über die 32 Zeichen danach gefunden), *chapter start only* (die
Stelle im Kapitel wurde nicht gefunden), *from the percentage only* (das
Kapitel fehlt in der anderen Datei, z. B. Calibres `titlepage.xhtml`).
`pull --write` bringt die Server-Position genauso in eine zugeordnete EPUB
auf dem Tolino zurück und in ein zugeordnetes kepub auf einem Kobo mit
Firmware 6.x als die Stelle, die Nickel speichert (`#kobo.N.M:offset`); in
ein kepub auf Kobo-Firmware 4.x schreibt es nichts.

**Kindle (experimentell).** Die Kindle-Unterstützung bleibt, wie sie ist,
bis Kobo und Tolino stabil laufen: keine Weiterentwicklung und kein
Gerätetest vor 0.1.0. `status` und `push` lesen auch die Bücher eines Kindle, und
zwar jedes KFX, das seiner EPUB zugeordnet ist (`map add`): Die zuletzt
gelesene Seite (`lpr` im `.sdr`-Ordner) wird in die EPUB übersetzt und
unter deren Prüfsumme gesendet. Auf den Kindle wird nichts geschrieben,
`pull` lehnt ab. Ein aktueller Kindle bietet nur MTP an, das der Mac nicht
als Laufwerk einhängt: Kopiere den Ordner `documents` oder nur die
`.kfx`-Datei eines Buchs samt `.sdr`-Ordner mit
[OpenMTP](https://openmtp.ganeshrvel.com/) in einen Ordner und gib diesen
als `--device` an. Lesen lassen sich nur Bücher, die per
**Send-to-Kindle** geschickt wurden; im Kindle-Shop gekaufte Bücher sind
verschlüsselt (DRM) und werden übersprungen. Ein Jailbreak ist nicht
nötig.

```sh
nickelinksync map add --device ~/Kindle-Kopie --book "Good Boy" --library ~/Calibre-Bibliothek
nickelinksync push --device ~/Kindle-Kopie
```

**Zuordnungen auf dem Gerät.** Das Plugin auf dem Gerät hat weder
Bibliothek noch Original-EPUBs; deshalb bereitet der Rechner das Gerät vor:

```sh
nickelinksync device prepare --device /Volumes/KOBOeReader --library ~/Calibre-Bibliothek
```

`device prepare` schreibt drei Dinge nach `.adds/NickelInkSync/` auf dem
Kobo oder Tolino und sonst nichts auf das Gerät (nie eine EPUB):

- `library.json`, den **Bibliotheksindex**: Titel, Autoren und Prüfsumme
  jeder Original-EPUB der Bibliothek;
- `mappings.json`, die Zuordnungen der Bücher auf diesem Gerät, dieselben
  wie in `map list`;
- `tables/<Prüfsumme>.json`, je zugeordnetem Buch eine
  **Übersetzungstabelle**: je Kapitel des Originals der Pfad, wo Absätze
  und Sätze beginnen und KOReaders Pfad zu jedem Textknoten (kein Text des
  Buchs), und wo der Text der Gerätedatei vom Original abweicht.

Damit synchronisiert die CLI auf dem Gerät (`--live`, vom Plugin
aufgerufen) ein zugeordnetes Buch unter der Prüfsumme des Originals und
übersetzt die Position wie der Rechner mit der Original-EPUB: Der
gesendete Absatz- oder Satzanfang (`position = paragraph` oder `sentence`)
ist derselbe; mit `position = exact` und für die Server-Position auf dem
Rückweg kann die Stelle innerhalb eines Satzes ein paar Zeichen abweichen
(„placed between the table's anchors“). Auf dem Gerät gilt:

| Buch | Synchronisiert |
|---|---|
| zugeordnet, mit Tabelle | unter der Prüfsumme der Original-EPUB, Position übersetzt |
| zugeordnet, Tabelle fehlt (am Gerät zugeordnet, seitdem nicht vorbereitet) | gar nicht („no-table“) |
| ein kepub ohne Zuordnung | gar nicht („not-mapped“) |
| im Index als Original oder Original einer Zuordnung | unter der eigenen Prüfsumme |
| nichts davon | gar nicht („not-mapped“) |
| Gerät nicht vorbereitet (keine `library.json`) | `pull` unter der eigenen Prüfsumme; **`push` sendet nichts** |

`device prepare` nach dem Zuordnen, Hinzufügen oder Umwandeln von Büchern
erneut ausführen. Eine am Gerät angelegte oder entfernte Zuordnung (im
Plugin geplant) wird in `mappings.json` am Rechner übernommen: Das
Original wird über seine Prüfsumme in der Bibliothek gefunden. Ein kepub,
das der Desktop-Client auf einen vorbereiteten Kobo kopiert, bekommt seine
Tabelle sofort. Eine Tabelle wird nur neu geschrieben, wenn sich die
Gerätedatei oder ihr Original geändert hat.

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
`$NICKELINKSYNC_DATA_DIR`), den sich CLI und Desktop-Client teilen.

### Desktop-Client (im Aufbau)

**macOS (ab 11, Apple Silicon und Intel):** `NickelInkSync-<Version>-macos.dmg`
öffnen und NickelInkSync in „Programme“ ziehen. Die App ist nicht
notarisiert (keine Apple Developer ID), daher blockiert macOS sie beim
ersten Start: einmal öffnen, dann Systemeinstellungen → Datenschutz &
Sicherheit → „Dennoch öffnen“ (seit macOS 15 genügt Rechtsklick → „Öffnen“
nicht mehr). Die CLI kommt als `nickelinksync-<Version>-macos.zip`; nach
dem Entpacken lässt `xattr -d com.apple.quarantine nickelinksync` sie
laufen. In der App steckt [kepubify](https://github.com/pgaskin/kepubify)
(MIT); seine Lizenz liegt in der App und auf dem Disk-Image.
`packaging/macos/package.sh` baut beide Dateien (CI: Job `package-macos`).

Ein Fenster für dieselbe Arbeit, auf derselben Bibliothek (`gui/`, egui):
`just gui` oder `cargo run -p nickelinksync-gui`. Es findet einen per USB
angesteckten Kobo oder Tolino (macOS: `/Volumes`), nimmt eine Kopie der
Bücher eines Kindle als Ordner und zeigt die Bücher danach, wie sie
synchronisiert werden (nicht zugeordnet, ohne Referenz-EPUB, zugeordnet,
eigene Prüfsumme), mit der Stelle auf dem Gerät; ohne Bibliothek warnt es,
dass es nicht prüfen kann, welche Bücher KOReader kennt. Dort ordnest du ein Buch auch zu: mit den
Vorschlägen der Bibliothek samt Bewertung oder einer selbst gewählten
EPUB; eine Zuordnung lässt sich ändern oder entfernen. Mit einem Server
in `config.ini` zeigt es die Stelle auf dem Server neben der auf dem
Gerät und sendet je Buch oder alle auf einmal; eine Rückfrage nach
`confirm` kommt als Dialog (Server behalten oder Gerät senden). Am
Tolino und am Kobo (ein kepub erst, wenn es zugeordnet ist, und nicht mit
installiertem Kobalt) schreiben „Holen“ und „Alle holen“ die Stelle vom Server in die
Gerätedatenbank, vorher legt es eine Sicherung im Datenordner an, wie
`pull --write`; eine Rückfrage kommt im selben Dialog (Gerät behalten oder
Server übernehmen), danach liest es das Gerät neu ein. Wirf das Gerät
aus, bevor du das Buch öffnest, und starte es neu, wenn die Meldung das
verlangt. „Verlauf“ zeigt, was CLI und Fenster
gesendet und geschrieben haben (`history.jsonl` im Datenordner).
„Aus Bibliothek hinzufügen“ kopiert Bücher der Bibliothek nach `Books/`
auf einem Kobo oder Tolino, nie über eine vorhandene Datei und nicht, wenn
dieselbe EPUB schon auf dem Gerät liegt; auf dem Kobo auf Wunsch als
kepub über [kepubify](https://github.com/pgaskin/kepubify) (gesucht im
`PATH`, in den Homebrew-Ordnern, in `~/go/bin` oder über
`$NICKELINKSYNC_KEPUBIFY`), gleich der EPUB zugeordnet.
„Über“ zeigt die Version und die Lizenzen (auch die von kepubify); einmal
am Tag fragt das Fenster bei GitHub nach einer neueren Version und nennt sie
oben mit Link (heruntergeladen wird nichts; in „Über“ abschaltbar).
„Einstellungen“ bearbeitet `config.ini` (Server, Benutzer, Gerätename,
`confirm`, `position`, Bibliothek) zeilenweise, Kommentare bleiben, vorher
legt es `config.ini.bak` an; ein dort eingegebenes Passwort landet nur als
Schlüssel im Schlüsselbund, und ein Passwort oder Schlüssel aus der Datei
lässt sich dorthin verschieben. „Übersicht“ zeigt alle angesteckten Geräte nebeneinander: eine Zeile je
Buch, je Gerät eine Spalte mit seiner Stelle, dazu die des Servers. Auf einen
Kobo oder Kindle schreibt es nichts, nur `mappings.json` und den
Verlauf. Beim Anstecken wird nichts
synchronisiert. Deutsch oder Englisch nach Systemsprache, umschaltbar.
`config.ini` und `mappings.json` teilt es mit der CLI; seine eigenen
Einstellungen (Sprache, Kindle-Ordner) liegen in `gui.json` im
Datenordner.

## Konfiguration

`push` und `pull` brauchen eine Konfigurationsdatei, `status` nutzt sie, wenn
es sie gibt. Standardort: `~/.config/nickelinksync/config.ini` (oder
`$XDG_CONFIG_HOME/nickelinksync/config.ini`, oder der Pfad in
`$NICKELINKSYNC_CONFIG`). Das Format steht im [englischen README](README.md#configuration):
`server`, `user`, `password` (oder `key` = MD5 des Passworts), optional
`device`, `device_id`, `confirm`, `notify` (nur für das Tolino-Plugin,
siehe unten), `library` (Ordner mit den Original-EPUBs für die
Buchzuordnung) und `position`. Für die `map`-Befehle genügt eine Datei, die
nur `library` (und `position`) enthält; `status` zeigt dann keinen Server.

`position` legt fest, welche Stelle `push` sendet. Das Gerät speichert, wo
seine Seite beginnt, oft mitten im Satz. KOReader öffnet die Seite, auf der
diese Stelle liegt, nach seinem eigenen Seitenumbruch, die Stelle kann also
irgendwo auf der Seite stehen. Ein Absatz- oder Satzanfang ist dort leichter
zu finden.

| Wert | Sendet |
|---|---|
| `paragraph` (Standard) | den Anfang des Absatzes; liegt der mehr als etwa eine halbe Seite (600 Zeichen ohne Leerzeichen) zurück, den Anfang des Satzes; liegt auch der so weit zurück, die Stelle selbst |
| `sentence` | den Anfang des Satzes, höchstens eine halbe Seite zurück |
| `exact` | die Stelle selbst, zeichengenau |

Die Ausgabe sagt, wie weit zurückgesetzt wurde („moved back 103 characters
to the paragraph start“), und `status` und `confirm` vergleichen mit
derselben Stelle; ein Buch gilt deshalb nicht als auf dem Gerät neuer.
`pull` übernimmt die Server-Position, wie sie ist.

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
Hat ein Buch eine [zuletzt abgeglichene
Position](#zuletzt-abgeglichene-position), gewinnt die Seite, die sich als
einzige bewegt hat, ohne Rückfrage; `confirm` entscheidet dann nur, wenn
sich beide bewegt haben (`always` fragt weiterhin jedes Mal).

Das Passwort steht nur in dieser Datei, nie in der Befehlszeile. Halte die
Datei privat (`chmod 600`); NickelInkSync warnt, wenn andere sie lesen
können. Unter macOS und Windows kann der Schlüssel stattdessen im
Schlüsselbund (Windows: Anmeldeinformationsverwaltung) liegen: Stehen
weder `password` noch `key` in der Datei, sucht die CLI dort (Dienst
„NickelInkSync“, Konto `<user>@<server>`). Die Einstellungen des
Desktop-Clients legen ihn dort ab; auf dem Gerät und unter Linux bleibt er
in der Datei.

## Plugin für den Tolino (experimentell)

Auf einem Tolino mit Firmware 5.x gleicht ein NickelHook-Plugin ohne
Rechner ab, per WLAN: Beim Öffnen eines seitengeladenen Buchs holt es eine
neuere Position vom Server, und das Buch öffnet dort; beim Verlassen des
Buchs sendet es deine. Es
ruft dasselbe Programm `nickelinksync` auf dem Gerät auf und folgt derselben
Regel `confirm`; fragen kann es noch nicht, eine Position, die eine Antwort
bräuchte, bleibt deshalb unverändert und steht im Log. Zuordnungen kennt das
Plugin nicht (sie liegen auf dem Rechner); ein kepub synchronisiert es
deshalb nie, die Meldung sagt dann „kepub ohne Zuordnung“. Auf einem Kobo
bleibt das Plugin vorerst inaktiv.

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
das Plugin es in `.adds/NickelInkSync/not-in-line.tsv`; die CLI übernimmt
den Vermerk beim nächsten Speichern in `state.json` (`not_in_line`) und
löscht die Datei. Ein solches Buch
wird nie gesendet, weder vom Plugin noch mit `push` am Rechner (außer mit
`--force`), weil die Stelle, die Nickel gespeichert hat, nicht deine ist;
der nächste `pull`, beim Öffnen oder per USB, übernimmt wieder die
Server-Position, auch wenn „zuletzt gelesen“ auf dem Tolino jetzt neuer
aussieht.

Jedes Ergebnis zeigt das Plugin als kurze Meldung unten auf dem Bildschirm
(vorerst auf Deutsch), etwa „Fortschritt übernommen“ mit Prozent und dem
Gerät, von dem die Position kam, oder „Laden fehlgeschlagen“ mit dem Grund
(keine Verbindung, Zeitüberschreitung, Anmeldung abgelehnt). Die Texte sind
kurz, weil der Tolino längere abschneidet. `notify` in `config.ini` legt fest, welche:

| Wert | Zeigt |
|---|---|
| `all` (Standard) | jedes Ergebnis, auch „Fortschritt ist aktuell“, wenn nichts zu übernehmen oder zu senden war (etwa wenn du ein Buch auf der Seite verlässt, auf der es mit der Server-Position geöffnet wurde) |
| `errors` | nur, was schiefging oder unterblieb: Server nicht erreicht, Position nicht übernommen, nicht gesendet |
| `none` | nichts; im Log steht weiterhin alles |

**Mit [Kobalt](https://github.com/nicoverbruggen/kobalt):** Kobalt, ein
Leseprogramm, das in Nickel läuft, schreibt die Lesepositionen selbst und
bekommt einen eigenen KOSync. Solange Kobalt installiert ist, schaltet sich
das Plugin ab: Es schreibt nichts in die Gerätedatenbank und sendet nichts,
sagt das einmal per Meldung („NickelInkSync ist aus“, etwa 20 Sekunden
nach dem Start des Tolino) und bei jedem Start im Log. Das Plugin bleibt installiert und arbeitet wieder, sobald Kobalt
entfernt ist (nach dem nächsten Start). Am Rechner verweigern `pull
--write` und „Holen“ im Desktop-Client ein Gerät mit Kobalt; `push` und
„Senden“ warnen nur. Ein übrig gebliebener Ordner `.adds/kobalt` ohne
seine Datei `uninstall` (so entfernt man Kobalt von Hand) zählt nicht.

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
  Bücher. Ein kepub wird erst synchronisiert, wenn es seiner Original-EPUB
  zugeordnet ist (siehe [Buchzuordnung](#buchzuordnung-im-aufbau)).

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
  von Nico Verbruggen (MIT), das Submodul des Plugins;
  [kepubify](https://github.com/pgaskin/kepubify) 4.0.4 von Patrick Gaskin
  (MIT), in der macOS-App mitgeliefert; dazu die Rust-Crates
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
