# Arbeitsliste NickelInkSync

Konkrete Schritte aus der [Roadmap](ROADMAP.md), in der Reihenfolge, in der wir
sie angehen. Ein Branch und ein PR pro Punkt (oder pro zusammengehörender
Gruppe), ein Commit pro erledigtem Schritt. Nach einer Unterbrechung: diese
Datei und `git status` / `git log` lesen, dann beim ersten offenen Punkt
weitermachen.

Status: `[ ]` offen · `[~]` in Arbeit · `[x]` erledigt · `[!]` blockiert (Grund steht dabei)

Wer: **du** = braucht dich (Gerät, Entscheidung, Zugang) · **ich** = kann ich allein erledigen

Erledigte Punkte wandern in der Roadmap nach „Recently completed“.

## Aktueller Stand (2026-11-07)

- **Fertig auf `main`:** Desktop-Client und CLI (macOS, Windows), Release-Workflow, Plugin C4/C9/C10 (Kobo 6.0, Kobo 4.x, Tolino).
- **0.1.0 kommt früher, ohne C5, C6, C7** (Entscheidung vom 2026-11-07); die drei stehen unter „Release 0.1.1“.
- **Erledigt am 2026-11-07:** Vorprüfung `device check`, Gerätetest-Tabellen, Release-Notes und READMEs; Gerätetests am Clara BW (6.0) und Libra 2 (4.38) bestanden.
- **Als Nächstes (du):** Release-Probelauf (Actions → Release → Run workflow), dann der Tag `v0.1.0` (Checkliste unten).
- **Was dich braucht:** die [Freigabe-Checkliste 0.1.0](#freigabe-checkliste-010), nichts anderes. Pro Gerät ein Termin (Tabellen in [`docs/device-tests/`](docs/device-tests/)).
- **Offen, nicht in 0.1.0:** PR #80 (Konfliktdialog, C5) bleibt offen und wartet auf 0.1.1.
- **Regeln, damit es übersichtlich bleibt:** höchstens zwei Sessions gleichzeitig; Gerätetests bündeln; Doku-Updates gesammelt.

**Reihenfolge der Geräte:** Erst wird alles, was mit dem **Tolino Shine Color
(5.18)** allein geht, umgesetzt und am Gerät getestet: P3, K4 rückwärts für
normale EPUBs, P2 am Tolino, N3, N4. Danach kommt der **Kobo** (Clara BW)
in einem eigenen Durchgang: P1-Durchlauf mit allen Geräten,
K4 rückwärts für kepub (`#kobo.N.M`), P2 am Kobo, N2. So geraten die
beiden Geräte beim Testen nicht durcheinander.

**Kobo auf Firmware 6.0 und 4.x:** Der Referenz-Clara-BW hat sich am
2026-10-28 selbst auf **6.0.279225** (Datenbankversion 222) aktualisiert
(Qt6 wie der Tolino, die Nickel-VM nutzt ebenfalls Kobo 6.0). Seit
2026-11-01 gibt es wieder ein 4.x-Gerät: einen **Kobo Libra 2 mit
4.38.23852** (Qt5). Kobo 4.x wird in 0.1.0 vollständig unterstützt (C8–C10
im Abschnitt Kobo-Client); bis dahin ist 4.x nur über die 4.46-Fixtures
geprüft. Bei Gerätetests immer das Gerät nennen: Libra 2 (4.x) oder Clara
BW (6.0).

**Danach zuerst D1, dann D2:** Sobald der Tolino-Durchgang abgeschlossen
ist, kommt die **manuelle Buchzuordnung in der CLI (D1)**, danach der
**Client mit grafischer Oberfläche (D2)**, mit höchster Priorität und vor
allem anderen (auch vor dem Kobo-Durchgang). Ohne die Zuordnung gibt es
weder den Sync kepub ↔ EPUB noch den Kindle-Sync. Wartung und Fehlerbehebung
dürfen jederzeit dazwischengeschoben werden. **Stand 2026-10-29:** D1 ist
fertig, D2 läuft (E0 bis E7 und O5 erledigt; als Nächstes der Release-Workflow). **Der
Kobo-Client** (Abschnitt Kobo-Client, C0–C10, geplant am 2026-10-31) gehörte
seit 2026-11-01 zu **Release 0.1.0**; seit 2026-11-07 kommen C5 bis C7 erst
in **0.1.1** (Abschnitt Release 0.1.1).

## Release 0.1.0

Entscheidungen des Besitzers (2026-10-31):

- Version **0.1.0**; Voraussetzung: D2 und die kepub-Rückübersetzung (PR #55)
  sind abgeschlossen.
- **Release-Assets:** der Desktop-Client für macOS (`.dmg`) und Windows
  (App als ZIP), dazu die CLI `nickelinksync` für macOS und Windows als
  eigene ZIPs, und `SHA256SUMS`. Dazu das **Gerätepaket** für Tolino und Kobo
  (`Kobo.tgz` für Kobo 6.x und Tolino, `KoboRoot.tgz` für Kobo 4.x, und
  `.adds/NickelInkSync/` mit dem ARM-Programm), als
  **experimentell** gekennzeichnet (geändert am 2026-11-01); den Workflow
  dafür baut die D2-Session.
- **Kein Linux:** E8 (AppImage) kommt im Release danach.
- **Kobo-Client** (C0–C10, Kobo und Tolino) gehörte zu 0.1.0 (Plan geändert
  am 2026-11-01); **geändert am 2026-11-07:** ohne C5, C6, C7, siehe unten.
- **Kobo 4.x vollständig in 0.1.0** (von dir am 2026-11-01, Testgerät Kobo
  Libra 2 mit 4.38.23852): Lesen und Rückschreiben per USB in CLI und
  Desktop-Client (C8) und das Plugin als **Qt5-Fassung** (C9, C10, Paket
  `KoboRoot.tgz`); die Qt6-Fassung für Kobo 6.x und Tolino 5.x bleibt.
- **Kindle:** bleibt in 0.1.0 (lesen, senden), als **experimentell**
  gekennzeichnet. Vor 0.1.0 keine weitere Kindle-Arbeit und kein
  Kindle-Test; E3b „Kindle-Kopie senden“, „Kindle per MTP lesen“ und L1
  ruhen, bis Kobo und Tolino stabil sind.
- **kepub:** synchronisiert nur über eine gültige Zuordnung, unter der MD5
  der zugeordneten EPUB, nie unter der eigenen Prüfsumme, mit oder ohne
  Bibliothek (PRs #55, #60). Aufs Gerät kommen die Zuordnungen mit `device
  prepare` (C3, PR #67); ohne sie sendet das Gerät nichts, auch der Tolino
  (gehört in die Release-Notes). Beim Kindle gilt dasselbe (nur zugeordnete
  KFX).
- Release-Notes **englisch, darunter deutsch**, wie die READMEs.
- Ein Workflow baut die Assets bei einem Tag `v*` und legt einen Entwurf an;
  den Tag setzt und das Release veröffentlicht nur der Besitzer.

**Früherer Schnitt (Entscheidung des Besitzers, 2026-11-07):** 0.1.0 erscheint
**ohne C5, C6 und C7**. Gründe: Du verlierst die Übersicht, und es soll kein
fehlerhaftes Plugin erscheinen. Umfang von 0.1.0:

- Desktop-Client und CLI für macOS und Windows;
- das Plugin für Kobo 6.0 (Qt6), Kobo 4.x (Qt5) und Tolino 5.18, alles
  **experimentell**: Öffnen übernimmt den Server-Stand, Schließen sendet,
  Vormerkliste, Meldungen; **kein Dialog bei Konflikten** (das Buch bleibt
  unverändert, nichts wird gesendet, Meldung und Logeintrag; aufgelöst wird
  am Rechner), kein „Jetzt synchronisieren“ und kein Zuordnen auf dem Gerät;
- Kindle experimentell wie bisher;
- unterstützt und getestet nur: Kobo 6.0.279225 (Clara BW), Kobo 4.38.23852
  (Libra 2), Tolino 5.18.279364 (Shine Color).

C5 (Konfliktdialog) liegt nur in PR #80 und bleibt dort; C5, C6, C7 stehen
jetzt unter „Release 0.1.1“.

### Freigabe-Checkliste 0.1.0

Das ist die einzige Stelle, an der du den Stand siehst. Je Gerät ein Termin;
die Schritte stehen in den Tabellen unter [`docs/device-tests/`](docs/device-tests/).

- [x] **(a) Tolino Shine Color (5.18):** (bestanden am 2026-11-08, Wartezeit 3000 ms) Retest mit dem Paket aus dem aktuellen
  `main` (wie C4 ohne Dialog: Server-Stand beim Öffnen übernehmen, offline
  lesen → vorgemerkt, offline öffnen ohne Lesen → nichts, anderes Buch
  öffnen → nachgeholt, Konflikt nicht geladen/nicht gesendet, ohne `device
  prepare`), dazu die längere Wartezeit „after opening (… ms)“ ·
  [`tolino-shine-color-5.18.md`](docs/device-tests/tolino-shine-color-5.18.md)
- [x] **(b) Kobo Libra 2 (4.38):** Kobalt-Schutz und „Gerät nicht
  vorbereitet“ (die zwei offenen C10-Punkte) · bestanden am 2026-11-07,
  dazu der volle Durchlauf mit Konflikt ·
  [`kobo-libra-2-4.38.md`](docs/device-tests/kobo-libra-2-4.38.md);
  **Clara BW (6.0)** dasselbe in Kurzform · bestanden am 2026-11-07 ·
  [`kobo-clara-bw-6.0.md`](docs/device-tests/kobo-clara-bw-6.0.md)
- [ ] **(c) Release-Probelauf:** GitHub → Actions → Release → Run workflow;
  Artefakte ansehen (Disk-Image, ZIPs, Gerätepaket mit `Kobo.tgz` und
  `kobo-4.x/KoboRoot.tgz`, `SHA256SUMS`)
- [x] **(d) Release-Notes** aktualisiert (`.github/release-notes.md`, READMEs)
  und von dir gelesen (2026-11-07, keine Änderungen)
- [ ] **(e) Tag `v0.1.0`** setzt nur du

## Release 0.1.1

Entscheidung des Besitzers (2026-11-07): C5, C6 und C7 kommen erst nach 0.1.0.
Inhalt, Abhängigkeiten und Status bleiben wie sie waren; die übrigen Etappen
(C0–C4, C8–C10) stehen im Abschnitt [Kobo-Client](#kobo-client).

- [ ] **C5 Dialog bei Konflikten** (braucht C4) · **PR #80 (offen, nicht auf `main`)**; Stand und Übergabe dort und in der Notiz `2026-11-02-kobo-c5-handover.md`
  - [ ] **ich:** Qt-Dialog beim Öffnen mit beiden Ständen und drei Knöpfen
    (`ConfirmationDialog` hat zwei eigene; der dritte muss ein
    Nickel-Widget sein, siehe C0).
  - [ ] **du:** Konflikt am Kobo nachstellen (KOReader und Kobo offline
    an verschiedenen Stellen), alle drei Knöpfe.
- [ ] **C6 Oberfläche** (braucht C5)
  - [ ] **ich:** Vormerkliste, „Jetzt synchronisieren“, Zuordnen am Gerät
    aus dem Index (Vorschläge nach Titel und Autor); Einstieg über
    NickelMenu-Datei und Buchmenü.
  - [ ] **du:** Am Kobo: vorgemerkte Bücher senden, ein kepub zuordnen.
- [ ] **C7 Tolino** (braucht C4 bis C6)
  - [ ] **ich:** Dasselbe Plugin auf dem Shine Color; Kobalts Schreibweg
    dort prüfen (Kobalt schreibt auf `develop` seit 8b144d2 auch auf dem
    Tolino, mit `tolino_bookmark`; für den Vergleich mit Kobalt daneben
    `ignore-kobalt` anlegen, N7). Wenn
    er geht, ersetzt er dort den heutigen Weg B (CLI schreibt, verwirft die
    Seiten im Cache, N5); sonst bleibt Weg B.
  - [ ] **du:** Dieselben Tests wie C4 bis C6 am Tolino.

## Release 0.2.0

Wünsche des Besitzers (2026-11-02) für den **Desktop-Client** (`gui/`).
Grundlage ist die Normenprüfung in `docs/usability-requirements.md`
(PR #79); die Nummern in Klammern verweisen auf deren Tabellen.

Entscheidungen des Besitzers (2026-11-02, jeweils die Empfehlung aus der
Normenprüfung):

- **Ziel WCAG 2.2 AA** statt 2.1 (E2): EN 301 549 V4.1.1 (September 2026)
  beruht darauf; 2.2 enthält 2.1 ganz.
- **Plattformen** (E1): macOS und Windows für 1.0.0; Linux nach Kräften.
- **Grenzen von egui** (E3: Tabellen, Überschriften, Live-Meldungen): in
  0.2.0 umgehen, bei egui/AccessKit melden oder beitragen, den Rest in den
  READMEs nennen; kein Wechsel weg von egui.
- **Tests** (E4): du mit der Sprachausgabe (Windows), ich mit VoiceOver
  (macOS); vor 1.0.0 ein Test mit einer blinden oder sehbehinderten Person.
- **Keine formale Konformitätserklärung** (E5), nur ein kurzer Abschnitt in
  den READMEs mit erreichter Stufe und bekannten Grenzen.
- **macOS-Menüleiste** (E6): in 0.2.0 nur Tastenkürzel (Cmd-, und Cmd-Q),
  ohne neue Abhängigkeit; ein echtes Menü, wenn es beim Testen fehlt.

- [~] **U1 Ergonomie und Usability** nach DIN EN ISO 9241 (Teile 110, 11,
  112, 125, 143, 210).
  - [x] **ich:** Normenprüfung (PR #79).
  - [x] **du:** Punkte für 0.2.0 und Entscheidungen festgelegt (2026-11-02).
  - [ ] **ich:** Verständliche Fehler (I6, A12): häufige Fehler (Server
    nicht erreichbar, falsches Passwort, Bibliothek fehlt, Gerät nur lesbar)
    deutsch und englisch, mit dem nächsten Schritt; Server-URL und
    Bibliotheksordner im Einstellungsdialog vor dem Speichern prüfen.
  - [ ] **ich:** Rückgängig und Abbrechen (I5): „Rückgängig“ in der Meldung
    nach dem Entfernen einer Zuordnung; Kopieren und Vorbereiten zwischen
    zwei Büchern abbrechen.
  - [ ] **ich:** Rückmeldung (I2, I7): Tooltips an Zuordnen, Entfernen,
    Senden und Holen in den Buchzeilen; Meldung mit Uhrzeit und
    Schließen-Knopf; Text neben dem Spinner („3 Bücher werden gesendet …“);
    Erklärung der Wörter in der Server-Spalte.
  - [ ] **ich:** Tastenkürzel unter macOS (I3, E6): Cmd-, öffnet die
    Einstellungen, Cmd-Q beendet; Windows entsprechend Strg-,.
  - [ ] **ich:** Hilfe (I4, A13): Knopf „Hilfe“ oben, öffnet den
    README-Abschnitt zum Client; Abschnitt „Barrierefreiheit“ in
    `README.md` und `README.de.md` (Tastatur, Zoom, Screenreader, Grenzen).
  - [ ] **du:** Nach diesen Punkten den Client mit drei Aufgaben ausprobieren
    (I11: Gerät einrichten, Buch zuordnen, Position holen) und sagen, wo es
    hakt.
- [~] **U2 Barrierefreiheit nach WCAG 2.2** (Stufe AA, sinngemäß auf
  Desktop-Software übertragen über WCAG2ICT und DIN EN 301 549, Kapitel 11):
  wenn möglich **spätestens in Release 1.0.0**. In 0.2.0 die Vorarbeit und
  die ersten Punkte:
  - [ ] **ich:** Beschriftungen mit den Feldern verbinden (A2,
    `labelled_by`): Einstellungen, Sprache, Suche im Kopierdialog,
    Pfadfelder.
  - [ ] **ich:** Kontrast (A5): eigene Farben für hell und dunkel
    (schwacher Text, Warnung, Fehler, Links) mit mindestens 4,5 : 1, an
    Screenshots nachgemessen.
  - [ ] **ich:** Tastatur und Fokus (A1, A9): obere Leiste in Leserichtung,
    Escape bricht den Rückfrage-Dialog ab, Enter löst die Hauptaktion aus,
    Fokus beim Öffnen in den Dialog und danach zurück.
  - [ ] **ich:** Zoom (A7): sichtbar machen und speichern; alle Dialoge bei
    200 % prüfen.
  - [ ] **ich:** Bestandsaufnahme mit VoiceOver (macOS) nach EN 301 549,
    Kapitel 11; Ergebnis in `docs/usability-requirements.md`.
  - [ ] **du:** Dieselbe Bestandsaufnahme mit der Sprachausgabe unter
    Windows (ich schreibe dir eine Checkliste).
  - [ ] **ich:** Für 1.0.0: Details zum gewählten Buch ohne Maus (A3),
    Tabellenzeilen mit eigener Beschriftung (A4), Meldungen vorlesen (A6),
    Hochkontrast unter Windows (A8), größere Knöpfe in den Buchzeilen
    (A10); Lücken von egui melden (E3).
  - [ ] **du:** Vor 1.0.0 ein Test mit einer blinden oder sehbehinderten
    Person (E4).

## Grundlagen

- [x] **G1 Lizenz** · PR #5
  - [x] **du:** Lizenz wählen: MIT, wie NickelHook und deine anderen Projekte.
    KOReader, koreader-sync-server und BookOrbit stehen unter AGPL-3.0; ihr
    Code dient nur als Vorlage, nachgebaut wird das beschriebene Verhalten.
  - [x] **ich:** `LICENSE` ersetzen, Abschnitt „Lizenz“ in beiden READMEs
    anpassen, Lizenzhinweis in `SCOPE.md`.
- [x] **G2 Rust-Grundgerüst** · PR #6
  - [x] **ich:** Crate `nickelinksync` (Bibliothek + Programm), `rustfmt.toml`,
    `clippy` ohne Warnungen, `Cargo.lock` im Repo.
  - [x] **ich:** CI-Job `check` um `cargo fmt --check`, `cargo clippy` und
    `cargo test` erweitern; die beiden bestehenden Prüfungen bleiben, der
    Jobname bleibt `check`.
- [x] **G3 ARM-Build** · PR #8
  - [x] **ich:** Statisch gelinktes Programm für
    `armv7-unknown-linux-musleabihf`, gebaut mit `cargo-zigbuild`
    (`just build-device`). Es hängt nicht von der glibc des Geräts ab und
    läuft daher auf Firmware 4.x und 5.x; `strayrose/nickeltc` bleibt für das
    Plugin (N2/N4). Der CI-Job `check` baut es, prüft es mit `file` und
    `qemu-arm` und lädt es als Artefakt `nickelinksync-armv7-musleabihf` hoch.
  - [x] **du:** Testlauf auf Clara BW (4.46) und Shine Color (5.18): Programm aus
    dem CI-Artefakt nach `.adds/NickelInkSync/` kopiert, `--version` gibt
    `nickelinksync 0.1.0` aus.
- [x] **G4 Testdaten** · PR #9
  - [x] **ich:** Skript, das aus einer `KoboReader.sqlite` nur die nötigen
    Tabellen und Zeilen übernimmt (seitengeladene Bücher und ihre Kapitel) und
    Titel, Autoren und Pfade ersetzt (`tools/make_db_fixture.py`), dazu
    Platzhalter-EPUBs mit der Struktur der drei Bücher
    (`tools/make_epub_fixture.py`) und ein Test, der die Fixtures lädt. Siehe
    [`tests/fixtures/README.md`](tests/fixtures/README.md).
  - [x] **du:** Gekürzte Daten vom Clara BW und Shine Color durchgesehen und
    freigegeben, bevor sie ins Repo kamen. Es ging um deine Lesedaten.

## Desktop-Client

- [x] **D1 CLI mit manueller Buchzuordnung** · PR #28, #31, #32, #33 ·
  höchste Priorität nach dem Tolino-Durchgang. Löst O1 ab und entblockt
  den Sync kepub ↔ EPUB und den Kindle-Sync (L1). In Etappen, je ein PR: (1) Speicher und Befehle,
  (2) Bibliothek mit Originalen und Vorschläge, (3) `status`/`push`/`pull`
  über die Zuordnung mit Positionsübersetzung, (4) Kindle in `status`/`push`.
  - [x] **ich:** Zuordnungsspeicher (`src/mapping.rs`): Buch auf dem Gerät
    (Kobo/Tolino: Prüfsumme der Gerätedatei; Kindle: `content_id`) ↔
    Original-EPUB (Pfad und Prüfsumme), `mappings.json` im Datenordner
    (`$NICKELINKSYNC_DATA_DIR`), Formatversion, atomar geschrieben;
    fehlende oder geänderte EPUB = kaputte Zuordnung. Begründung der
    Schlüssel in `SCOPE.md` (Book identity).
  - [x] **ich:** `map add|list|remove`; Kindle-Bücher per USB ohne
    Jailbreak (`src/kindle_device.rs`). Tests mit Book A (kepub ↔ EPUB),
    gleicher Datei und den Kindle-Fixtures (`tests/mapping.rs`).
  - [x] **du:** Etappe 1 am Kobo 6.0 ausprobiert (2026-10-28): Book R
    kepub ↔ EPUB, `map add`, `map list`. Heat 2 vom Tolino steht noch aus.
  - [x] **ich:** Bibliothek mit Originalen (`library =` in `config.ini`,
    `--library`, `src/library.rs`): Calibre-`metadata.db` nur lesend
    (`immutable=1`), sonst `*.epub` rekursiv mit Titel/Autor aus der OPF;
    nie hineinschreiben (Test vergleicht die Dateien vorher und nachher).
  - [x] **ich:** `map suggest` (`src/suggest.rs`): Titel (auch gegen den
    Dateinamen, für Send-to-Kindle), Autor und Kapitel, jede Teilbewertung
    sichtbar; Bücher mit gleicher Prüfsumme wie in der Bibliothek brauchen
    keine Zuordnung. `map add` ohne `--epub` fragt im Terminal nach dem
    Vorschlag. Die Kindle-`content_id` wählt das Buch aus, steht aber nicht
    in der Bibliothek und zählt deshalb nicht zur Bewertung. Tests mit
    selbst erzeugter Calibre-`metadata.db` (`tests/library.rs`).
  - [x] **du:** Etappe 2 mit deiner Calibre-Bibliothek und dem Kobo 6.0
    geprüft (2026-10-28): drei kepubs aus Calibre, je der richtige Band
    vorn (100 %, 99 %, 91 %). Calibre ersetzt beim Umwandeln die
    Cover-Seite und teilt große Kapitel (`_split_NNN`).
  - [x] **ich:** Etappe 3 (`src/transfer.rs`): `status`/`push`/`pull`
    über die Zuordnung, unter der Prüfsumme der Original-EPUB; Position
    über Kapitelpfad (Calibres `_split_NNN` zusammengeführt) und sichtbaren
    Text übersetzt, sonst Kapitelanfang bzw. Prozent, in der Ausgabe
    benannt. Mit Bibliothek wird ein Buch ohne Zuordnung, das weder in der
    Bibliothek liegt noch Original einer Zuordnung ist, nicht
    synchronisiert; ohne Bibliothek wie bisher. `pull --write` übersetzt
    zurück in eine zugeordnete EPUB auf dem Tolino und schreibt nie in ein
    kepub. Tests: Markierungen Book P/Q kepub ↔ EPUB, Kobo-6.0-Position
    in Book R, Calibre-artige Umwandlung von Book R, Mock-Server
    (`tests/transfer.rs`, `tests/sync_mapping.rs`).
  - [x] **du:** Etappe 3 am Gerät (2026-10-28): Vampire Hunter D
    (Calibre-kepub mit geteilten Kapiteln) auf dem Kobo 6.0 bis „the events
    of the day“ gelesen; die gesendete Stelle stimmt zeichengenau, KOReader
    zeigt sie aber weit unten auf seiner Seite.
  - [x] **ich:** `position = paragraph | sentence | exact` in
    `config.ini` (Standard `paragraph`): `push` sendet den Absatzanfang,
    höchstens etwa eine halbe Seite zurück, sonst den Satzanfang; Ausgabe
    nennt die Verschiebung (`transfer::step_back`).
  - [x] **du:** Mit `position = paragraph` geprüft (2026-10-28): Vampire
    Hunter D sendet den Absatzanfang (`p[26]`, 103 Zeichen zurück);
    KOReader blieb auf derselben Seite, passt.
  - [x] **ich:** Etappe 4: Kindle in `status`/`push` (nur lesend, ohne
    Jailbreak), `pull` lehnt ab. Der Paperwhite bietet am Mac nur MTP: die
    CLI liest eine Kopie (Ordner mit `documents/` oder mit `.kfx` und
    `.sdr`, z. B. mit OpenMTP). KFX-Leser für echte Send-to-Kindle-Bücher
    erweitert: eingebettete Schriften (`$418`), Läufe mit Offset in
    `$609`, geschützte Leerzeichen und `<br/>` als `\n`, Cover-Abschnitt
    und geteilte Kapitel (Block über den Text gesucht). Gekaufte Bücher
    (DRM) werden übersprungen. Geprüft an Kopien von „Good Boy“ und „Der
    wiedergefundene Freund“ (alle bzw. fast alle Blöcke exakt).
  - [x] **du:** Kindle-Test (2026-10-29): „Good Boy“ (Send-to-Kindle, 16
    Abschnitte gegen 14 Kapitel) bis 14 % gelesen, per OpenMTP kopiert,
    `map add` über den Vorschlag, `push`: KOReader öffnet dieselbe Seite
    („In the end it wasn't the stone“ oben rechts).
  - [ ] **ich (später, vor oder in D2):** Kindle direkt per MTP lesen
    (libmtp), ohne Kopie.
- [~] **D2 Desktop-Client mit grafischer Oberfläche** · direkt nach D1.
  Baut auf derselben Bibliothek auf: Geräte und Bücher anzeigen, Zuordnung
  anlegen, Sync auslösen. In Etappen E0–E8, je ein PR (Plan vom
  2026-10-29, unten).
  - [x] **du:** Plattformen festgelegt: zuerst **macOS**, dann **Windows**,
    Linux optional, dann für möglichst viele Distributionen und Versionen.
  - [x] **du:** Funktionsumfang der ersten Version festgelegt (2026-10-24):
    - **Geräte:** Tolino und Kobo (lesen, zurückschreiben, sobald es in der
      CLI fertig ist), Kindle nur lesen (Position zum Server).
    - **Sync:** Positionen anzeigen, Push/Pull per Knopf mit Rückfrage nach
      der `confirm`-Einstellung, wie in der CLI. Kein automatischer Sync
      beim Anstecken.
    - **Zuordnung:** Vorschläge nach Titel und Autor (Kindle: `content_id`),
      du bestätigst oder wählst selbst. Bücher ohne Zuordnung und ohne
      gleiche Prüfsumme stehen in einer Liste „Nicht zugeordnet“ und werden
      nicht synchronisiert.
    - **EPUB-Originale:** aus einem konfigurierbaren Bibliotheksordner, der
      **Calibre-Bibliothek** (nur lesend, `metadata.db` als unveränderliche
      SQLite-Datei).
    - **Bücher aufs Gerät kopieren:** aus der Calibre-Bibliothek auf Tolino
      oder Kobo. Beim Kobo bietet die App an, das Buch mit **kepubify**
      (MIT) nach kepub umzuwandeln. Beim Kopieren legt die App die
      Zuordnung selbst an (kepub ↔ Original-EPUB), weil sie beide Dateien
      kennt; das Tolino-EPUB bleibt unverändert und hat dieselbe Prüfsumme.
    - **Einstellungen:** Dialog für Server, Benutzer, Passwort, Gerätename
      und `confirm`; das Passwort liegt im Schlüsselbund (macOS) bzw. in
      der Windows-Anmeldeinformationsverwaltung, die Datei hält nur den
      Benutzernamen.
    - **Gemeinsame Daten:** CLI und GUI teilen Konfiguration und
      Zuordnungen.
    - **Oberfläche:** normales Fenster, Deutsch und Englisch nach
      Systemsprache, umschaltbar.
    - **Außerdem:** Geräteansicht nebeneinander (O5), Sicherungskopie der
      Gerätedatenbank per Knopf zurückspielen, Verlauf der Syncs,
      Hinweis auf ein neues Release (keine Selbstaktualisierung).
  - [x] **du:** kepubify wird **mitgeliefert**: ein Binary je Plattform
    (macOS, Windows, Linux im AppImage), mit dem MIT-Lizenzhinweis von
    kepubify in der App und im Paket.
  - [x] **du:** Plan und offene Punkte entschieden (2026-10-29): macOS ab
    **11** als Universal-Binary (arm64 + x86_64); bis E5 testest du per
    `cargo run`, das `.dmg` kommt in E6; App-Name **NickelInkSync**, das
    Icon entwerfe ich als SVG, du gibst Rückmeldung; im Schlüsselbund liegt
    der **MD5-Schlüssel**, nicht das Passwort; Hinweis auf ein neues
    Release höchstens einmal am Tag über die GitHub-API, abschaltbar;
    Releases per Tag (`v0.2.0`) mit `.dmg`, später `.exe` und dem
    Tolino-Paket; gemeinsamer Verlauf `history.jsonl` für CLI und GUI;
    Windows als **ZIP** (`.exe`, `kepubify.exe`, Lizenzen) statt
    Installer; für E7 testest du auf deinem Windows-Gaming-PC.
  - Architektur: Das Paket `nickelinksync` (Bibliothek und CLI) bleibt,
    wie es ist; die GUI kommt als zweites Paket `gui/`
    (`nickelinksync-gui`) in einen Cargo-Workspace und hängt nur von der
    Bibliothek ab. Der Gerätebau (`-p nickelinksync`) und das
    Tolino-Paket bleiben ohne egui. Lange Vorgänge laufen in einem
    Arbeits-Thread (Befehle und Ereignisse über Kanäle). Die Rückfrage
    `confirm` ist Teil der Daten (`session::Action::Ask`): Die CLI fragt im
    Terminal, die GUI im Dialog, beide wenden dieselben Schritte an.
    Einstellungen nur für die GUI (Sprache, Kindle-Ordner,
    Update-Prüfung) in `gui.json` im Datenordner, weil `config.ini`
    unbekannte Schlüssel ablehnt.
  - [x] **ich:** **E0** Sync-Ablauf in die Bibliothek (`src/session.rs`):
    `Session`/`KindleSession`, `Server`, `Identity`, je Buch
    `push_step`/`pull_step` mit `Action` (`InSync`, `Apply`, `Keep`,
    `Ask`), `take`, `write`, `sync::outlook` für `status`; nichts darin
    gibt aus oder fragt. Die CLI gibt aus und fragt wie bisher: Ihre
    Ausgabe ist über alle Aufrufe der Testsuite zeichengleich geblieben.
    Tests ohne CLI in `tests/session.rs`.
  - [x] **ich:** **E1** Workspace und Fenster, das nur anzeigt: angesteckte
    Kobo/Tolino (`/Volumes/*` mit `.kobo/KoboReader.sqlite`), Kindle-Ordner
    wählen, Bücher nach gleicher Prüfsumme, zugeordnet und „Nicht
    zugeordnet“, Position auf dem Gerät; Arbeits-Thread; Deutsch/Englisch
    (`sys-locale`, umschaltbar). CI: clippy/test mit `--workspace`,
    Gerätebau mit `-p nickelinksync`, eigener Job `gui-macos` (kein
    Pflicht-Check). eframe 0.37; Ordnerdialog (`rfd`) nur für macOS und
    Windows, unter Linux wird der Pfad eingetippt. `just gui` startet.
  - [x] **du:** E1 am Mac mit Kobo, Tolino und der Kindle-Kopie
    ausprobiert (`just gui`, 2026-10-29): Bücher und Status stimmen, auch
    im Kindle-Ordner; Sprachwahl wirkt und bleibt nach dem Neustart. Am
    Tolino nur Book R, weil Paradise Lost und Good Boy Shop-Bücher sind
    (siehe P4). Der gewählte Kindle-Ordner ließ sich nicht wieder
    entfernen: jetzt Knopf „Kindle-Ordner entfernen“.
  - [x] **du:** Knopf geprüft (2026-10-29): Der Ordner verschwindet und
    bleibt nach dem Neustart weg.
  - [x] **ich:** **E2** Zuordnung in der GUI: Vorschläge mit Bewertung,
    bestätigen oder EPUB selbst wählen, entfernen. Die Logik von
    `map add`/`map suggest` liegt jetzt in der Bibliothek (`src/map.rs`:
    `device_books`, `open_library`, `candidates`, `add`), die CLI gibt nur
    noch aus (Ausgabe zeichengleich). In der GUI: „Zuordnen …“ bei Büchern
    ohne Zuordnung, „Ändern …“ und „Entfernen“ (mit Rückfrage) bei
    zugeordneten und kaputten; der Dialog zeigt die Kandidaten mit
    Bewertung (der beste ab 80 % vorausgewählt, darunter „unsicher“), die
    gleiche Datei in der Bibliothek, oder ohne Bibliothek nur „EPUB wählen
    …“. Geschrieben wird nur `mappings.json`, nie das Gerät.
  - [x] **du:** E2 am Kobo ausprobiert (2026-10-29): „Acts of Contrition“
    (neues kepub) in der GUI zugeordnet, klappt. Der Test zeigte zwei
    Schwächen, beide behoben: Ohne `library =` standen ungeprüfte Bücher
    unter „Gleiche Datei“ (jetzt „Eigene Prüfsumme“ mit Grund, auch dort
    „Zuordnen …“), und die Knöpfe lagen bei langen Pfaden außerhalb des
    Fensters (jetzt neben dem Titel).
  - [x] **ich:** Auf deinen Wunsch: Ohne Referenz-EPUB gibt die App einen
    Hinweis statt einer falschen Meldung. Ohne Bibliothek steht oben eine
    Warnung, ungeprüfte Bücher stehen in der Gruppe „Ohne Referenz-EPUB“
    (Warnfarbe, „Zuordnen …“), „Eigene Prüfsumme“ zeigt nur Geprüftes
    (gleiche Datei wie in der Bibliothek, Original einer Zuordnung, Letzteres
    auch ohne Bibliothek); der Dialog sagt „Keine Referenz-EPUB gefunden“.
    Die CLI sendet ohne Bibliothek weiter unter der eigenen Prüfsumme; ob
    die GUI solche Bücher beim Push überspringt oder fragt, entscheiden wir
    in E3.
  - E3 neu geschnitten (2026-10-29, wegen des Nutzungslimits) in drei
    kleine PRs:
  - [x] **ich:** **E3a** Verlauf: `src/history.rs`, `history.jsonl` im
    Datenordner (eine JSON-Zeile je gesendeter oder geschriebener
    Position, Richtung, Gerät, Titel, Dokument, Stelle, Prozent, `by`).
    Die CLI schreibt nach `push` (Kobo/Tolino und Kindle) und nach
    `pull --write`, nicht auf dem Gerät (`--live`); Ausgabe unverändert.
  - [x] **ich:** **E3b** GUI: Server-Positionen in der Bücherliste (Spalte
    „Server“ mit Prozent, Gerät, Zeit und Aussage nach `sync::outlook`),
    „Senden“ je Buch und „Alle senden“ über `Session::push_step`; Fragen
    (`Action::Ask`) sammeln und in einem Dialog beantworten (Gerät,
    Server, Überspringen), dann die gemerkten `Update`s senden; Bücher
    ohne Referenz-EPUB überspringt „Alle senden“, „Senden“ am einzelnen
    Buch fragt vorher; Verlauf (`by: "gui"`) als Fenster. Kindle über
    `KindleSession::push_step`. Tests in `gui/` mit dem Mock-Server aus
    `tests/common` (`#[path]`). Umgesetzt in `gui/src/sync.rs`: nach dem
    Einlesen fragt die App den Server (Spalte „Server“: Prozent, Gerät,
    „gleich“/„Gerät neuer“/„Server neuer“/„fragt vorher“); „Senden“ je
    Buch, „Alle senden“ oben; Fragen kommen in einen Dialog mit „Server
    behalten“ (vorgewählt) oder „Gerät senden“; danach fragt sie den
    Server erneut. Knopf „Verlauf“ oben rechts (letzte 200 Einträge, von
    CLI und GUI). Ohne Server in `config.ini` ein Hinweis statt der
    Knöpfe.
  - [x] **du:** E3b: Book R am Tolino senden, einmal mit Rückfrage
    (KOReader weiter vorn), Verlauf ansehen (2026-10-31, PR #41).
  - [ ] **du:** E3b: Kindle-Kopie senden (später, getrennt).
  - [x] **ich:** **E3c** Holen und Rückschreiben am Tolino in der GUI
    (`pull_step`, `take`, `write` mit Sicherungskopie, gleicher Dialog),
    Kobo nur „zum Server“, bis P2 für den Kobo fertig ist. Umgesetzt:
    „Holen“ je Buch und „Alle holen“ oben, nur am Tolino und nur für
    zugeordnete Bücher und solche mit eigener Prüfsumme; Rückfragen im
    Dialog mit „Gerät behalten“ (vorgewählt) oder „Server übernehmen“;
    danach liest die App das Gerät neu ein; die Meldung nennt die
    Sicherung und erinnert ans Auswerfen. Verlauf mit `by: "gui"`. Zwei
    Schreibvorgänge in derselben Sekunde bekommen getrennte Sicherungen
    (`…-2.sqlite`), statt dass der zweite scheitert. Nach dem Holen einer
    Stelle mitten im Absatz galt das Buch als „Server neuer“, weil der
    Rückweg zum XPointer an den Absatzanfang rückt; jetzt gilt es als
    gleich, wenn auf dem Gerät genau das steht, was Holen schreiben würde
    (`Session::against`, für `status`, `push`, `pull` und die GUI). Bei
    „gleich“ sind „Senden“ und „Holen“ ausgegraut; „Alle senden“ und „Alle
    holen“ lassen solche Bücher aus.
  - [x] **ich:** Geräteansicht nebeneinander (O5), wie entschieden mit
    einer Spalte je Gerät: Knopf „Übersicht“ oben, ein Fenster mit einer
    Zeile je Buch (KOSync-Prüfsumme, also auch kepub und Kindle-Kopie in
    derselben Zeile), je angestecktem Gerät eine Spalte mit Prozent (Zeit
    und XPointer beim Darüberfahren), dazu die Spalte „Server“. Nur Bücher,
    die synchronisiert werden; „Neu laden“, nach Senden und Holen lädt es
    von selbst neu.
  - [ ] **du:** O5: Tolino und Kobo zusammen anstecken, „Übersicht“ öffnen.
  - [x] **du:** E3c: Book R vom Server auf den Tolino holen, auswerfen,
    öffnen (2026-10-31): Holen schreibt die richtige Stelle, danach „gleich“
    und ausgegraute Knöpfe. Nickel öffnet nur am Kapitelanfang (R2-1 statt
    R2-19), das ist N5 (PR #40), nicht E3c.
  - [x] **ich:** **E4** Einstellungen; Schlüssel im Schlüsselbund bzw. in
    der Anmeldeinformationsverwaltung (`keyring`, nur für macOS und
    Windows, nicht im Gerätebau). Die CLI sucht `password`/`key` in der
    Datei, dann im Schlüsselbund; die GUI bietet an, ein Passwort aus der
    Datei dorthin zu verschieben. Umgesetzt: Knopf „Einstellungen“
    (Server, Benutzer, Passwort, Gerätename, `confirm`, `position`,
    Bibliothek); `config.ini` wird zeilenweise geändert (Kommentare,
    Reihenfolge und `notify`/`device_id` bleiben), vorher `config.ini.bak`,
    Rechte 600. Ein eingegebenes Passwort wird nur als MD5-Schlüssel
    gespeichert (Schlüsselbund, Dienst „NickelInkSync“, Konto
    `<user>@<server>`; ohne Schlüsselbund als `key =` in der Datei);
    ändern sich Server oder Benutzer, zieht der Schlüssel mit. `keyring`
    3.6.3 nur für macOS und Windows. Tests mit einem Ordner statt des
    Schlüsselbunds (`NICKELINKSYNC_KEYCHAIN_DIR`).
  - [x] **du:** E4: Einstellungen öffnen, „In den Schlüsselbund
    verschieben“, danach `status` in der CLI (2026-10-31, PR #49).
  - [x] **ich:** **E5a** Bücher aus Calibre auf Tolino oder Kobo kopieren,
    beim Kobo auf Wunsch mit kepubify, Zuordnung entsteht dabei. Nichts
    auf dem Gerät überschreiben. Umgesetzt: Knopf „Aus Bibliothek
    hinzufügen …“ (Suche nach Titel, Autor, Datei; Auswahl per Haken);
    Ziel `Books/`; eine Datei gleichen Namens oder dieselbe EPUB (gleiche
    Prüfsumme) in `Books/` bleibt und wird gemeldet; geschrieben wird mit
    `create_new`. kepubify (bis E6 nicht mitgeliefert) wird im `PATH`, in
    `/opt/homebrew/bin`, `/usr/local/bin`, `~/go/bin` oder über
    `NICKELINKSYNC_KEPUBIFY` gesucht; die kepub wird gleich der EPUB
    zugeordnet. Bibliothek: `nickelinksync::copy`.
    Beim Test gefunden: Der Kobo mit Firmware 6.0 hat auch die
    `tolino_*`-Tabellen und galt deshalb als Tolino (kein kepub-Haken,
    dafür „Holen“ am Kobo). Jetzt entscheidet das Modell in
    `.kobo/version`, dann der Händler in `.kobo/affiliate.conf` (`Kobo`
    oder z. B. `Thalia.de`), erst zuletzt die Tabellen.
    Entscheidung (2026-10-31): „Holen“ auch am Kobo wie am Tolino, vorerst
    nur für EPUBs; kepubs folgen mit der Rückübersetzung (K4, eigene
    Session). Mit K4 (PR #55) holt der Desktop-Client auch zugeordnete
    kepubs.
  - [x] **du:** E5a: ein Buch auf den Tolino kopieren, eins als kepub auf
    den Kobo (2026-10-31): Tolino als EPUB, Kobo als kepub und gleich
    zugeordnet; am Kobo „Holen“ bei EPUBs, bei kepubs nur „Senden“.
  - [x] **ich:** **E5b** Sicherungskopie der Gerätedatenbank zurückspielen
    (Bibliothek, CLI `restore`, Dialog im Fenster). Umgesetzt:
    `nickelinksync::restore` und `Device::restore` über SQLites
    Backup-API, vorher eine Sicherung des jetzigen Stands; abgelehnt werden
    die Sicherung eines anderen Geräts (`.device`-Datei mit Prüfsumme der
    Seriennummer, ab jetzt neben jeder Sicherung), eine andere
    `DbVersion`, ein `-journal` und Kobalt. Ältere Sicherungen gelten als
    „Gerät unbekannt“. Danach auswerfen und neu starten.
  - [ ] **du:** E5b: am Tolino eine Stelle holen, dann „Sicherung
    zurückspielen …“ mit der eben angelegten Sicherung; auswerfen, neu
    starten, Buch öffnen (alte Stelle).
  - [x] **ich:** **E6a** macOS-Paket: `packaging/macos/package.sh` baut
    `NickelInkSync-<Version>-macos.dmg` (Universal-App arm64 + x86_64,
    ab macOS 11, ad hoc signiert, nicht notarisiert, mit `/Applications`-
    Verknüpfung und Lizenzen) und `nickelinksync-cli-<Version>-macos.zip`
    (Universal-CLI). kepubify 4.0.4 (MIT) liegt neben dem Programm in der
    App, SHA-256 der beiden macOS-Binaries im Skript fest; die App sucht
    kepubify zuerst neben sich. Icon-Entwurf `packaging/icon.svg`. CI-Job
    `package-macos` (kein Pflicht-Check) lädt beide als Artefakt hoch.
    README: Gatekeeper (seit macOS 15 „Dennoch öffnen“ in den
    Systemeinstellungen), `xattr` für die CLI.
  - [x] **du:** Icon ansehen; `.dmg` installieren (2026-10-31): Variante A
    gewählt (zwei E-Reader und ein Smartphone im Sync-Kreis), bis 32 Pixel
    das erste Icon mit einem E-Reader; Gatekeeper hat nichts verhindert;
    das Icon stimmt auch bei laufender App.
  - [x] **ich:** **E6b** Hinweis auf neue Releases (höchstens einmal am Tag
    über die GitHub-API, abschaltbar) und Fenster „Über“ mit Version und
    Lizenzen (NickelInkSync, kepubify). Umgesetzt: `/releases/latest`
    (ohne Entwürfe und Vorabversionen), 10 s Zeitlimit, ohne Release (404)
    kein Hinweis; Zeitpunkt und gefundene Version in `gui.json`, der
    Hinweis bleibt, bis diese Version installiert ist; oben ein Link
    „Neue Version … verfügbar“. Schalter in „Über“.
  - [x] **du:** E6b: „Über“ öffnen, Lizenzen ansehen (2026-11-01): Lizenzen
    werden korrekt angezeigt.
  - [x] **ich:** **E7** Windows: Pfade für Konfiguration und Daten
    (`%APPDATA%`, heute nur `HOME`/XDG), Wechsellaufwerke erkennen,
    Anmeldeinformationsverwaltung, ZIP aus der CI (Windows-Runner); ohne
    Code-Signing warnt SmartScreen beim ersten Start.
    Umgesetzt: Konfiguration in `%APPDATA%\nickelinksync\config.ini`,
    Daten (Zuordnungen, Sicherungen, Verlauf, `gui.json`) in
    `%LOCALAPPDATA%\nickelinksync`, `~` ist `%USERPROFILE%`; Laufwerke D:
    bis Z: (gab es schon); Schlüssel in der Anmeldeinformationsverwaltung
    (`keyring`, aus E4); kein Konsolenfenster im Release-Build; Icon
    (`packaging/windows/icon.ico`, bis 32 Pixel das kleine) und Version in
    der exe (`gui/build.rs`, `winresource`). `packaging/windows/package.ps1`
    baut `NickelInkSync-<Version>-windows-x64.zip` (mit kepubify 4.0.4,
    SHA-256 geprüft) und `nickelinksync-cli-<Version>-windows-x64.zip`; CI-Job
    `windows` (clippy, Tests, Paket, kein Pflicht-Check). Lokal quer
    gebaut und mit clippy geprüft (`x86_64-pc-windows-gnu` über zig).
  - [ ] **du:** E7: ZIP aus der CI auf dem Windows-Rechner entpacken,
    SmartScreen „Trotzdem ausführen“, Tolino anstecken, Einstellungen,
    ein Buch senden; Icon in Taskleiste und Explorer ansehen.
  - [x] **ich:** **Release-Workflow** (nach E7): GitHub-Actions-Workflow,
    der bei einem Tag `v*` die Release-Assets baut und als **Entwurf**
    eines GitHub-Release anlegt; Versionsprüfung Tag ↔ `Cargo.toml`
    beider Crates; `SHA256SUMS` mit hochladen; Release-Notes englisch,
    darunter deutsch, mit Hinweisen zu Gatekeeper und SmartScreen. Den Tag
    setzt und das Release veröffentlicht nur der Besitzer.
    Umgesetzt: `.github/workflows/release.yml` (Jobs `version`, `macos`,
    `windows`, `device`, `draft`); Notes-Vorlage `.github/release-notes.md`
    mit Download-Tabelle, Installation (Gatekeeper, SmartScreen,
    Gerätepaket experimentell, Kobalt), kepub- und Kindle-Hinweis. Das
    Gerätepaket `NickelInkSync-<Version>-device.zip` (`Kobo.tgz`,
    `.adds/NickelInkSync/`, Lizenz) ist dabei. Von Hand gestartet ein
    Probelauf ohne Entwurf. Mit actionlint (und shellcheck) geprüft.
  - [ ] **du:** nach dem Merge einen Probelauf starten (Actions → Release
    → Run workflow) und die Artefakte ansehen.
  - [ ] **du:** E7 auf dem Windows-PC mit Tolino oder Kobo.
  - [ ] **ich (optional):** **E8** Linux als **AppImage**, gebaut auf einer
    alten glibc-Basis (ältestes noch unterstütztes Ubuntu LTS), damit es auf
    möglichst vielen Distributionen und Versionen läuft; keine Abhängigkeit
    außer Grafik- und Fensterbibliotheken (X11/Wayland, OpenGL), die jede
    Desktop-Installation hat. Ein statischer musl-Build geht für die GUI
    nicht, weil die Grafiktreiber zur Laufzeit geladen werden.
  - Nicht in D2: Kindle direkt per MTP (nach der ersten Version, siehe D1;
    am Mac kollidiert libmtp mit `ptpcamerad`, Windows braucht WPD);
    Rückschreiben am Kobo (Kobo-Durchgang).

## Kern

- [x] **K1 Gerätedatenbank lesen**
  - [x] **ich:** Nur lesend öffnen: `immutable=1` für Kopien und per USB
    angeschlossene Geräte (`db::DeviceDb::open_snapshot`, am Tolino geprüft:
    Datei unverändert, keine Journal-Dateien); `mode=ro` auf dem Gerät,
    während Nickel läuft (`--live`, `db::DeviceDb::open_live`).
  - [x] **ich:** Seitengeladene Bücher auflisten (`ContentType = 6`,
    `file:///mnt/onboard/`), Position aus `ChapterIDBookmarked`,
    `___PercentRead`, `DateLastRead` lesen.
  - [x] **ich:** Kapitel über die OPF-Spine und den `href` aus
    `ChapterIDBookmarked` finden (relativ zur EPUB-Wurzel), nicht über das
    Format der Kapitel-`ContentID` (Kobo: Pfad, Tolino: UUID). `BookID` +
    `VolumeIndex` nur auf dem Kobo als Gegenprobe (`ContentType = 9` =
    Spine-Index); auf dem Tolino sind die Kapitelzeilen ToC-Einträge,
    `VolumeIndex` ist dort der ToC-Index (siehe `SCOPE.md`).
  - [x] **ich:** Firmware aus `.kobo/version` lesen (drittes Feld), Tolino an der
    Datenbank erkennen (`tolino_*`-Tabellen). Dazu das Modell aus dem letzten
    Feld (bekannt: Clara BW, Shine Color).
- [x] **K2 Prüfsumme**
  - [x] **ich:** Teil-MD5 wie KOReader: 1-KiB-Stichproben ab Offset **0**
    (nicht 256), dann 1 KiB, 4 KiB, … (`hash::partial_md5`).
  - [x] **du:** `metadata.epub.lua` aus KOReader geliefert, für die drei
    Fixture-EPUBs (`tests/fixtures/koreader/`). Sie enthalten
    `partial_md5_checksum` als Vergleichswert und die Positionen für K4.
  - [x] **ich:** Test gegen diese Werte (alle sieben EPUB-Fixtures, `tests/device.rs`).
- [x] **K3 Position auf Kapitelebene**
  - [x] **ich:** `ChapterIDBookmarked` zerlegen: `href#kobo.N.M` (kepub),
    `href#point(...)` (EPUB) und `href` ohne Fragment (Kapitelanfang)
    (`position::Position::parse`).
  - [x] **ich:** OPF-Spine lesen, `href` → `DocFragment[n]` (`epub::Epub`).
    Prozentwert für KOSync: `___PercentRead / 100`.
  - [x] **ich:** Tests mit den Fixtures aus G4: Heat 2 (Kobo und Tolino:
    `xhtml-0-81.xhtml` = Spine-Index 81 → `DocFragment[82]`; auf dem Tolino
    hat die Kapitelzeile `VolumeIndex` 80) und Titan 2 (Spine-Index 39, ohne
    ToC-Eintrag und ohne Kapitelzeile).
- [~] **K4 Position genau**
  - [x] **ich:** Aus der Datei ablesen, welchen DOM-Pfad KOReader erwartet
    (Leerraum, `koboSpan`-Elemente): KOReader behält `koboSpan` und die
    kepub-Hüllen, fasst Leerraum zusammen und zählt Zeichen; der Adobe-Point
    zählt UTF-8-Bytes (`SCOPE.md`).
  - [x] **ich:** Kobo-Span → XPointer und Adobe-Point → XPointer
    (`position::to_xpointer`): alle 48 Markierungen von Kobo und Tolino in
    Book P und Book Q ergeben genau KOReaders XPointer.
  - [x] **ich:** Rückwärts für den Tolino: XPointer → Adobe-Point
    (`position::from_xpointer`). Rundreise: alle 16 Tolino-Markierungen in
    Book P und Book Q kommen unverändert zurück, KOReaders Markierungen
    derselben Wörter ergeben genau die Tolino-Points, ebenso Book R. `pull`
    zeigt im Probelauf, was auf dem Gerät gespeichert würde (`on device`).
  - [x] **du:** Am Tolino geprüft (2026-10-24): `pull --force` übersetzt
    KOReaders Server-Position in Book R (Anfang R2-63) in
    `OEBPS/text/chapter2.xhtml#point(/1/4/128/1:0)`, wie erwartet. Ob
    Nickel dort öffnet, prüft P2.
  - [x] **ich (C1):** Rückwärts für den Kobo: XPointer →
    Kobo-Span mit Offset (`#kobo.N.M:offset`, UTF-16-Einheiten,
    `position::from_xpointer_kepub`). Rundreise: alle 16 Kobo-Markierungen
    in den kepubs von Book P und Book Q kommen unverändert zurück,
    KOReaders Markierungen derselben Wörter ergeben genau die Kobo-Spans,
    ebenso die Leseposition `#kobo.17.1:204` in Book R. Eine Stelle
    außerhalb jedes Spans (Leerraum zwischen Absätzen) geht zum nächsten
    Span. `pull --write` schreibt in ein zugeordnetes kepub am Kobo 6.x;
    der Seiten-Cache aus PR #48 vergleicht Spans genauso wie Points.
    Kobo 4.x (Span ohne Offset, Adobe-Point mit zusammengefasstem Text)
    wird seit **C8** ebenfalls geschrieben (siehe dort).
  - [x] **du (Entscheidung, 2026-10-31):** Ein kepub wird **nur über eine
    Zuordnung** zu seinem EPUB synchronisiert, immer unter der Prüfsumme
    des EPUBs, nie unter der eigenen (auch ohne Bibliothek; ein
    `.kepub.epub` auf dem Tolino ebenso). Folge: Das Plugin auf dem Gerät
    kennt keine Zuordnungen und überspringt kepubs, bis die Zuordnungen
    aufs Gerät kommen.
  - [x] **du:** Am Kobo geprüft (2026-10-31, Clara BW 6.0.279225): KOReader bei
    R3-130 auf den Server, `pull --write` schrieb ins kepub
    `chapter3.xhtml#kobo.131.1` und in die EPUB
    `chapter3.xhtml#point(/1/4/262/1:0)`, ließ 11 zwischengespeicherte
    Seiten fallen; nach dem Neustart öffnen kepub und EPUB an der
    richtigen Stelle. Im kepub weiter bis R3-140, `push` sendet unter der
    EPUB-Prüfsumme `p[139]`, KOReader öffnet dort. Mit installiertem
    Kobalt schreibt `pull --write` nichts, wie vorgesehen.
  - [ ] **ich (Kobo-Durchgang):** Positionsformat und Kapitelzeilen auf
    Kobo 6.0 prüfen (kepub-Span, Adobe-Point: zählt 6.0 Kommentare als
    Knoten wie der Tolino?); die CLI nimmt ab Firmware 5 das
    Tolino-Verhalten an (`PointFlavor::for_firmware`), ungeprüft.
  - [ ] **du:** Fixture-Bücher (Book P, Q, R, je kepub und EPUB) auf den
    Kobo 6.0 laden, Book P und Q markieren wie zuvor, dann
    Datenbankkopie für neue Fixtures (`tools/make_db_fixture.py`).
  - [x] **ich:** Zweites Probebuch für Umlaute, Zeichen mit 3 und 4 Bytes,
    CDATA, Leerraum am Absatzanfang und kombinierende Akzente, unter eigenem
    Namen (**Book Q**, `book-q*.epub`), damit es nicht mit Book P verwechselt
    wird.
  - [x] **du:** Book Q auf Kobo, Tolino und in KOReader laden und die acht
    Zielwörter markieren (Liste in `tests/fixtures/README.md`). Auf dem Kobo
    erst im zweiten Anlauf vollständig gespeichert.
  - [x] **du:** Book P (`tests/fixtures/epub/book-p*.epub`) auf Kobo, Tolino
    und in KOReader laden und die acht Zielwörter markieren (Liste in
    `tests/fixtures/README.md`), dann beide Datenbanken und die `.sdr`-Ordner
    kopieren.
  - [x] **ich:** Markierungen auslesen (`make_db_fixture.py --bookmarks`) und
    mit den erwarteten Offsets vergleichen (Ergebnis in
    `tests/fixtures/README.md`).
  - [x] **du:** Gegenprobe: Ein von uns berechneter XPointer öffnet in KOReader
    die richtige Stelle. Mit Book P (2026-10-24) nur das Kapitel prüfbar, weil
    das Buch etwa eine Bildschirmseite lang ist. Mit Book R (2026-10-24):
    Tolino-Seite beginnt in R1-156 bei Zeichen 253 („spät ruhig Garten.“),
    `push` sendet `…/p[156]/text()[1].253`, KOReader öffnet die Seite, die in
    R1-156 bei Zeichen 44 beginnt und bis R1-160 reicht, also mit der
    Stelle darauf; beide 27 %. Die Seiten beginnen verschieden, weil
    KOReader anders umbricht; die Position selbst stimmt.
  - [x] **ich:** Langes Testbuch **Book R** (`tools/make_long_epub.py`): drei
    Kapitel mit je 200 Absätzen, jeder Absatz mit Marke `R<Kapitel>-<Absatz>`
    (Anleitung in `tests/fixtures/README.md`).
  - [x] **du:** Book R auf den Tolino und in KOReader geladen, auf dem Tolino
    mitten in Kapitel 1 geblättert, `push`, in KOReader Fortschritt geholt
    (Ergebnis oben).
- [x] **K5 KOSync-Client**
  - [x] **ich:** Statt Docker ein kleiner Mock-Server in den Tests
    (`tests/common/mod.rs`): läuft ohne Netz und ohne echtes Konto, auch in
    der CI.
  - [x] **ich:** Anmeldung (`x-auth-user`, `x-auth-key` = MD5 des Passworts),
    Fortschritt lesen (`GET`) und schreiben (`PUT`) (`kosync::Client`).
    HTTPS über `rustls` (ring, eingebaute Stammzertifikate), kein OpenSSL;
    der statische ARM-Build läuft weiter. TLS gegen kosync.eu geprüft
    (Anmeldung mit Dummy-Konto wird mit 401 abgelehnt).
  - [x] **du + ich:** Erster echter `push` gegen kosync.eu mit deinem Konto
    (Book P und Book R vom Tolino, 2026-10-24). Dabei gefunden: kosync.eu
    antwortet ohne gespeicherte Position mit `[]` statt `{}`.
  - [x] **du:** Server: **kosync.eu** (Konto vorhanden). Das Dashboard dort
    zeigt den Lesestand je Hash und kann Testwerte wieder löschen. Das
    Passwort gibst du nur in die lokale Konfiguration ein, nie in den Chat
    oder ins Repo; automatische Tests laufen gegen einen Test-Server ohne
    echtes Konto.

## Programm

- [~] **P1 `status`, `push`, `pull`**
  - [x] **ich:** Konfigurationsdatei `~/.config/nickelinksync/config.ini`
    (Server, Benutzer, Passwort oder MD5-Schlüssel, optional Gerätename und
    -ID; Format im README). Das Passwort steht nicht in der Befehlszeile;
    Warnung, wenn andere die Datei lesen können.
  - [x] **ich:** `status`: Position auf dem Gerät und auf dem Server
    gegenüberstellen, mit Hinweis, welche Seite neuer ist.
  - [x] **ich:** `push`: Position zum Server. Eine neuere Server-Position
    (Server-Zeitstempel nach `DateLastRead`) wird nur mit `--force`
    überschrieben; `--dry-run`, `--book`.
  - [x] **ich:** `pull`: nur anzeigen, was sich ändern würde (Probelauf).
    Die neueste Position gewinnt; rückwärts nur nach Rückfrage (P3).
  - [x] **du:** Passwort in die lokale Konfiguration eingetragen, `push` vom
    Tolino Shine Color, Buch in KOReader mit Sync geöffnet: landet an der
    richtigen Stelle (Book R, siehe K4).
  - [ ] **du:** Ein Durchlauf mit Kobo, Tolino und KOReader am selben Buch.
- [x] **P2 Rückschreiben per USB** (erst Tolino, Kobo im Kobo-Durchgang)
  - [x] **ich:** `pull --write` (`db::Device::write_positions`):
    Sicherungskopie nach `~/.local/share/nickelinksync/backups/`, dann in
    einer Transaktion `ChapterIDBookmarked` und `___PercentRead` setzen;
    `DateLastRead` bleibt, außer es ist neuer als der Server, dann eine
    Sekunde vor dem Server-Zeitstempel (Begründung in `SCOPE.md`). Die
    Tolino-Datenbank ist im WAL-Modus: exklusive Sperre, Checkpoint nach dem
    Commit, danach keine `-wal`/`-shm`/`-journal`-Dateien. Schreibt nur mit
    `.kobo/version`. An einer Kopie der Shine-Color-Datenbank geprüft.
  - [x] **du:** Am Tolino geprüft (2026-10-24, Book R): KOReader-Seite
    beginnt am Ende von R2-134, `pull --write` schreibt
    `chapter2.xhtml#point(/1/4/270/1:269)`, keine Journal-Dateien übrig.
    Nach dem Auswerfen öffnet Nickel die Seite mit dieser Stelle (oben
    R2-133) und speichert beim Schließen den eigenen Seitenanfang
    (`#point(/1/4/268/1:46)`, 57 %). **Nickel übernimmt die Position.**
  - [x] **ich (C1):** dasselbe für den Kobo, EPUB und zugeordnetes kepub
    (PR #55); am Clara BW (6.0) geprüft 2026-10-31, öffnet nach dem
    Neustart an der Stelle (siehe K4).

- [x] **P3 Rückfrage bei abweichender Position (einstellbar)** · PR #18
  - [x] **ich:** `confirm = never | backwards | always` in der Konfiguration
    (`sync::decide`). „Rückwärts“: Die Position, die überschrieben würde,
    liegt weiter hinten im Buch (Spine-Index, im selben Kapitel die Stelle
    im Text, Prozent nur als Rückfall; seit P2, weil Nickel und KOReader
    den Prozentwert verschieden berechnen), nicht nach Zeitstempel, weil
    Nickel `DateLastRead` schon beim Öffnen setzt. Gilt
    für `push` und `pull`; im Terminal Frage „[d]evice or [s]erver?“, ohne
    Terminal wird nichts überschrieben und das Buch gemeldet; `--force`
    (jetzt auch bei `pull`) überschreibt ohne Frage. Später im Plugin als
    Dialog (O3/O4).
  - [x] **du:** Standardwert festgelegt: `backwards`.
  - [x] **du:** Am Tolino geprüft (2026-10-24, Book R): KOReader bei 45 %
    in Kapitel 2 synchronisiert (10:21:06), Buch auf dem Tolino nur
    geöffnet (`DateLastRead` 10:21:28, Position 27 % in Kapitel 1).
    `status` meldet „push asks first“, `push` fragt, Antwort `s` behält die
    Server-Position; mit `< /dev/null` wird nichts gesendet.

- [ ] **P4 Shop- und Cloud-Bücher ohne DRM** (nach D2) · Bisher werden nur
  seitengeladene Bücher (`file:///mnt/onboard/…`) synchronisiert, weil
  Shop-Bücher als DRM-Bücher galten. Am Tolino (2026-10-29) liegen
  heruntergeladene Shop-Bücher ohne Verschlüsselung in
  `.kobo/kepub/<ContentID>` (Paradise Lost, Good Boy), Position wie bei
  seitengeladenen EPUBs (`#point(...)`). Beim Herunterladen legt der
  Tolino `META-INF/cdp.info` (Bestellnummer) ins EPUB, die Prüfsumme
  weicht also immer von der eigenen Kopie ab.
  - [ ] **ich:** DRM sicher erkennen: `encryption.xml` nur, wenn
    Textdateien verschlüsselt sind (nicht bei verschleierten Schriften),
    `rights.xml` (Adobe), `license.lcpl` (LCP), Kobo-Store-Schlüssel in
    `content_keys`; zusätzlich muss das erste Kapitel als XHTML lesbar
    sein, sonst überspringen und nennen.
  - [ ] **ich:** Shop-Bücher in `status`/`push` und in der GUI: immer über
    eine Zuordnung zur Original-EPUB, Schlüssel ist die **ContentID**
    (wie die Kindle-`content_id`), weil sich die Datei beim erneuten
    Herunterladen ändern kann; ohne Zuordnung nicht synchronisiert, auch
    ohne Bibliothek. `SCOPE.md` anpassen („Out of scope“: Bücher mit DRM
    statt Shop-Bücher).
  - [ ] **du:** Good Boy auf dem Tolino zuordnen, `push`, in KOReader
    öffnen.
  - [ ] **ich + du (später):** Rückschreiben erst nach einem Test, ob der
    eigene Cloud-Sync von Tolino/Kobo die geschriebene Stelle
    überschreibt (Flugmodus, dann WLAN). Das Plugin bleibt bis dahin bei
    seitengeladenen Büchern.

## Plugin

- [x] **N1 Symbole für Kobo 4.x (Qt5)** · in **C9** (Abschnitt
  Kobo-Client) aufgegangen; Ergebnis in
  [`docs/nickel-4.38-symbols.md`](docs/nickel-4.38-symbols.md)
  - [x] **du:** Download des Firmware-Updates 4.38.23852 für den Libra 2
    erlaubt (2026-11-01), siehe C9.
  - [x] **ich:** Buch geöffnet (`ReadingController::setVolume`), verlassen
    (`ReadingView::leavingReadingExperience`) und Springen
    (`ReadingView::goToBookmark`) gibt es auf 4.38 unter denselben Namen.
- [x] **N2 Plugin für den Kobo 6.0** (statt Qt5 für 4.x) · in **C4**
  (Abschnitt Kobo-Client) aufgegangen, PR #72
  - [x] **ich:** Symbole aus `libnickel.so` von Kobo 6.0 prüfen (Vorbild
    N3, Tolino 5.18) und das Tolino-Plugin (Qt6) für den Kobo bauen;
    Paket `Kobo.tgz` (C0, C4).
  - [x] **du:** Test auf dem Clara BW (6.0): der Gerätetest von C4.
- [x] **N3 Symbole für Tolino 5.18 (Qt6)** · Ergebnis in
  [`docs/nickel-5.18-symbols.md`](docs/nickel-5.18-symbols.md)
  - [x] **du:** `libnickel.so` aus dem Kobo-Build 5.18.270971 und die
    Symbolliste des Tolino-Builds 5.18.264769 bereitgestellt.
  - [x] **ich:** Buch geöffnet: `ReadingController::setVolume(Volume
    const&, Bookmark const&)`; Buch verlassen:
    `ReadingView::leavingReadingExperience()` (speichert die Position
    vorher selbst). Signaturen und Rückgabetypen per Disassembly geprüft,
    alle Symbole in der Tolino-Liste.
  - [x] **ich:** Die Datenbank allein reicht im Plugin nicht: Beim Öffnen
    hat Nickel das Buch schon gelesen und schreibt seine Fassung beim
    Blättern zurück. Stattdessen im Öffnen-Hook eine Kopie des `Volume`
    mit neuer Position übergeben (`Content::setChapterIdBookmarked`,
    `setPercentRead`); Rückfall `ReadingView::goToBookmark`. Am Gerät
    noch zu prüfen (N4).
  - [x] **ich (K1-Rest, N4):** Auf dem Gerät die Datenbank mit `mode=ro`
    lesen, nicht `immutable=1` (`--live`): Im WAL-Modus stünde Nickels
    letzte Änderung sonst noch in der `-wal`-Datei und fehlte.
- [x] **N4 Plugin Qt6** · PR #22
  - [x] **ich:** NickelHook-Plugin (`plugin/`, Submodul
    `nicoverbruggen/NickelHook`, Branch `qt6-support`): Öffnen-Hook holt die
    Position (`pull --live --emit`, höchstens etwa 15 s) und setzt sie in
    das Buch, das Nickel öffnet, Verlassen-Hook sendet sie im Hintergrund
    (`push --live`). Nur Tolino, nur mit Programm und `config.ini` in
    `.adds/NickelInkSync/`; Log `NickelInkSync.log` daneben.
  - [x] **ich:** CLI auf dem Gerät: `--live` (`mode=ro`, nie schreiben),
    `--content-id`, `--timeout`, `pull --emit` (K1-Rest).
  - [x] **ich:** Build im Qt-6.5.2-Image
    `ghcr.io/dkuester/nickelstorygraph-toolchain` (nicht `strayrose/nickeltc`:
    das ist Qt 5 und baut kein Plugin für 5.x), Paket `Kobo.tgz`, `just
    plugin` / `just package`. CI-Schritt im Job `check`: Symbole gegen die
    5.18-Liste, ARM, nur Qt 6, Handler exportiert, Artefakt
    `NickelInkSync-tolino`.
  - [x] **du:** Erster Test auf dem Shine Color (2026-10-24, Book R):
    Plugin aktiv, `pull` holt R3-45 (75 %) richtig, Nickel öffnet aber nur
    das Kapitel (R3-1): Beim Öffnen lädt Nickel das Buch noch einmal aus
    der Datenbank (`VolumeManager::getById`) und nimmt die Stelle im Kapitel
    von dort. Der `push` danach (R3-13, hinter dem Server) wurde nach der
    Regel `backwards` korrekt zurückgehalten.
  - [x] **ich:** Zusätzlicher Hook auf `VolumeManager::getById`, nur
    während des Öffnens und nur für dieses Buch: setzt die Position auch
    im neu geladenen Buch (Weg 1, von dir gewählt).
  - [x] **du:** Zweiter Test (2026-10-24): `getById`-Hook greift, Nickel
    öffnet trotzdem am Kapitelanfang (R3-1 statt R3-130). Der `push` beim
    Verlassen scheiterte, weil kein WLAN da war (`failed to lookup
    address`); „Jetzt synchronisieren“ ist die Tolino-Cloud und hatte nur
    nebenbei das WLAN eingeschaltet.
  - [x] **ich:** WLAN selbst einschalten (`WirelessWorkflowManager::
    connectWireless`, wie NickelStorygraph): beim Öffnen höchstens 12 s
    warten, beim Verlassen im Hintergrund bis 45 s. Diagnose im Log: jeder
    `getById` des Buchs mit Aufrufer, wo Nickel 3 s und 10 s nach dem Öffnen
    steht, und die gespeicherte Position in `pull` (Weg A, von dir gewählt).
  - [x] **du:** Dritter Test (2026-10-24): WLAN-Einschalten und `push`
    funktionieren (R3-188 kam an, KOReader zeigt es in der letzten Zeile).
    Öffnen weiter am Kapitelanfang: ein `getById` beim Öffnen, gepatcht,
    Nickel steht trotzdem bei `#point(/1/4/2/1:0)`. Im geladenen `Volume`
    kommt nur das Kapitel an, nie die Stelle darin.
  - [x] **du:** Weg C gewählt: nach dem Öffnen mit Nickels eigener
    Navigation springen (`ReadingView::goToBookmark`); Weg B (Datenbank
    schreiben, während Nickel läuft) nur, wenn C nicht geht.
  - [x] **ich:** Sprung eingebaut: warten, bis das Buch offen ist,
    Nickels Lesezeichen für die aktuelle Stelle nehmen, Start auf die
    geholte Position setzen (`ContentLocation::create`, RMSDK-Format aus
    `#point(`), `goToBookmark`.
  - [x] **du:** Vierter Test (2026-10-24, `confirm = never` zum Testen):
    KOReader-Push R2-63 kam richtig an, Tolino-Push R2-113 auch. Gesprungen
    wurde nicht: `ReadingView::isBookOpened()` blieb 15 s lang falsch,
    obwohl das Buch offen war. (Erster Versuch scheiterte an einer
    angeklebten Zeile in `config.ini`, mein Fehler in der Anleitung.)
  - [x] **ich:** „Offen“ heißt jetzt: Lesesicht aktiv und zweimal hintereinander
    dieselbe Stelle gemeldet.
  - [x] **du:** Fünfter Test (2026-10-24): Sprung gar nicht versucht, weil das
    WLAN beim Öffnen nicht innerhalb von 12 s kam; der WLAN-Dialog (Auswahl
    des Netzes) hat die Lesesicht verlassen und einen `push` ausgelöst.
  - [x] **ich:** Umgebaut (von dir gewünscht): Buch öffnet sofort, im
    Hintergrund WLAN (erst still, nach 15 s mit Nickels WLAN-Dialog, wie von
    dir angeregt), `pull`, Sprung; kein `push`, solange das läuft. Die
    Änderungen am `Volume` und der `getById`-Hook sind entfernt.
  - [x] **du:** Sechster Test (2026-10-24): WLAN und `pull` in einer Sekunde,
    `goToBookmark` meldet „done“, Nickel bleibt aber in Kapitel 2 (Ziel
    Kapitel 1). Danach hat der `push` die neuere KOReader-Position auf dem
    Server mit der Tolino-Position überschrieben.
  - [x] **ich:** Kein `push` mehr, wenn Nickel nach dem Sprung nicht im
    geholten Kapitel steht; `pull --live` schreibt nicht mehr „Dry run“.
  - [x] **du:** Siebter Test (2026-10-24): Sprung innerhalb von Kapitel 2
    (R2-121 nach R2-170): wieder „done“, Nickel bleibt stehen. Weg C ist
    damit gescheitert. Der `push` danach hat R2-170 erneut überschrieben,
    weil die Sperre dagegen erst im neuen Plugin war, das ich dich nicht
    installieren ließ (mein Fehler).
  - [x] **ich:** Weg B (vorher von dir freigegeben, falls C scheitert): vor
    dem Öffnen still WLAN (höchstens 10 s), `pull --live --write --emit`
    schreibt die Position in die Zeile des Buchs, während Nickel läuft
    (SQLite-Sperren, Nickels WAL bleibt, eine Sicherung am Tag mit SQLites
    Backup-Funktion in `.adds/NickelInkSync/backups/`), dazu die
    `Volume`-Kopie für das Kapitel. Kein `push` für ein Buch ohne Abgleich
    beim Öffnen oder außerhalb des geholten Kapitels.
  - [x] **du:** Achter Test (2026-10-24): `isInternetAccessible()` blieb
    10 s falsch, obwohl das WLAN verbunden war; das Buch öffnete
    unverändert, und die Sperre hat den `push` danach richtig verhindert.
  - [x] **ich:** Nickels Internet-Meldung entscheidet nur noch, wie lange
    gewartet wird; `pull` wird immer versucht. Ohne Antwort vom Server kein
    `push`; ohne Neueres auf dem Server ist das Buch abgeglichen und wird
    gesendet (vorher wurde es fälschlich nie gesendet). Braucht `pull` eine
    Antwort (`NICKELINKSYNC_UNANSWERED`), wird ebenfalls nicht gesendet.
  - [x] **du:** Neunter Test (2026-10-24, wieder `confirm = backwards`):
    Der KOReader-Push R3-49 kam nicht auf dem Server an, der Tolino war
    damit neuer und blieb bei R2-121 (richtig nach der Regel). Deine Frage
    nach dem Renderer war berechtigt: Der Tolino rendert mit **TENG**, nicht
    RMSDK; deshalb griffen meine Eingriffe in Nickels Speicher nicht.
  - [x] **du + ich:** Zehnter Test (2026-10-24): Server-Stand vorher geprüft
    (R3-49 von macOS, 16:08 UTC). Beim Öffnen geschrieben und gesichert,
    Nickel öffnet auf der Seite mit R3-49 (oben R3-48). Weitergelesen bis
    R3-58, beim Verlassen gesendet, KOReader zeigt es. **Weg B funktioniert.**
  - [ ] **ich (später, O3/O4):** Rückfrage im Plugin als Dialog, statt eine
    Position, die eine Antwort braucht, liegen zu lassen.
- [~] **N5 Meldungen im Plugin** · PR #25
  - [x] **ich:** Nickels Toast (`MainWindowController::toast`, per
    Disassembly geprüft: `void`, das `int` ist die Anzeigedauer in ms) für
    jedes Ergebnis beim Öffnen und Verlassen, vorerst nur auf Deutsch.
    Neuer Schlüssel `notify = all | errors | none` (Standard `all`); die CLI
    prüft ihn nur, das Plugin liest ihn selbst.
  - [x] **ich:** CLI nennt das Ergebnis in Maschinenzeilen:
    `pull --emit` zusätzlich `NICKELINKSYNC_FROM`, `NICKELINKSYNC_NOT_TAKEN`
    und `NICKELINKSYNC_ERROR<tab>Art<tab>Meldung` (timeout, network, auth,
    server, other); neu `push --emit` mit `SENT`, `IN_SYNC`, `NOT_SENT`,
    `UNANSWERED`, `ERROR`.
  - [x] **ich:** `push` läuft als asynchroner `QProcess` statt losgelöst
    über `/bin/sh`; das Verlassen wartet weiter nie, die Ausgabe landet im
    Log, sobald der Push fertig ist.
  - [x] **du:** Gerätetest der Meldungen (2026-10-24): übernommen,
    gesendet, aktuell, kein Server und `notify = errors` erscheinen richtig;
    `QLocale()` ist `de_DE` (System `en_US`).
  - [x] **ich:** Der Test zeigte zwei Schwächen: Ohne WLAN hieß der Grund
    „Zeitüberschreitung“ (jetzt „Kein WLAN oder Server nicht erreichbar“),
    und „übernommen“ kam, obwohl Nickel am Kapitelanfang öffnete. Die
    Meldung kommt jetzt erst nach der Prüfung, und die zählt nur noch die
    Seite mit der geholten Stelle, nicht das ganze Kapitel.
  - [x] **ich + du:** Öffnen im selben Kapitel: Nickel öffnet am
    Kapitelanfang (R3-81 geholt, R3-1 geöffnet; Zeile vorher R3-58).
    Per USB öffnet es im selben Kapitel richtig (du, 2026-10-25: R3-91 →
    R3-121), also lag es an Nickels Speicher. Jetzt bekommt auch das
    `Volume`, das Nickel übergibt, die Position: **funktioniert** (du,
    2026-10-25: R3-152 geholt, Nickel auf der Seite mit R3-151, Meldung
    „übernommen“).
  - [x] **ich:** Direkt nach dem Übernehmen wieder verlassen meldet
    „Fortschritt nicht gesendet / Braucht eine Bestätigung“: Nickel speichert
    den Seitenanfang (R3-150), der vor der Server-Stelle (R3-152) liegt, und
    `confirm = backwards` fragt. Richtig nach der Regel, aber lästig. Jetzt
    merkt sich das Plugin die Seite, auf der es nach dem Übernehmen steht;
    wer auf ihr verlässt, sendet nichts („Fortschritt ist aktuell / Noch
    auf der Seite vom Server“). Wer weiterblättert, sendet wie bisher.
  - [x] **du:** Gerätetest dazu (2026-10-25): Das Übernehmen misslang
    wieder (R3-1 statt R3-170), obwohl auch Nickels `Volume` gesetzt war.
    Muster über alle Läufe: Das **erste** Öffnen eines Buchs nach dem Start
    von Nickel misslingt, das zweite klappt. Das Gleich-Kapitel war Zufall.
    Die Sicherungen griffen, aber danach galt das Gerät als neuer (Nickel
    hatte R3-1 mit neuer Zeit gespeichert); nur `confirm = backwards`
    verhinderte, dass R3-8 gesendet wurde.
  - [x] **ich (A):** Dauerhafte Sperre `.adds/NickelInkSync/not-in-line.tsv`:
    Das Plugin trägt ein Buch ein, wenn Nickel es nicht an der geholten
    Stelle öffnet, und streicht es, wenn es klappt. Die CLI sendet es nie
    (auch nicht am Rechner, außer `--force`), und `pull` übernimmt die
    Server-Position ohne Rücksicht auf Nickels `DateLastRead`, ohne
    Rückfrage, solange die Zeile noch Nickels Stelle hält.
  - [~] **du:** Gerätetest der Sperre (2026-10-25, `594c477`): Diesmal
    klappte das Übernehmen beim ersten Öffnen nach dem Start (R3-179 geholt,
    Seite mit R3-178), also wurde die Sperre selbst nicht ausgelöst.
    Verlassen auf der übernommenen Seite sendete nichts („aktuell“),
    Weiterlesen bis R3-186 sendete. Die Sperre bei einem misslungenen
    Öffnen ist am Gerät noch nicht getestet.
  - [~] **ich:** Ursache der misslungenen Übernahmen: Nickels
    `Volume`-Cache. Per Disassembly geprüft: `VolumeManager::getById` liest
    nur den Cache, sobald es ihn gibt, nie die Datenbank; was die CLI von
    außen schreibt, sieht Nickel also nicht. Jetzt leert das Plugin nach dem
    Schreiben den Cache (`Qt6RepositoryAccess<Volume>::purgeCache`, wie
    Nickels eigene Migrationen nach einem SQL-Schreiben, unter Nickels
    Mutex; `Repository::purgeCache<Volume>` fehlt im Tolino-Build), lädt
    das Buch mit `getById` neu (das füllt den Cache aus der Datenbank) und
    öffnet mit diesem `Volume`. Das Log zeigt Cache-Stand vorher und
    nachher und 3 s nach dem Öffnen die Stelle. PR #26.
  - [x] **du:** Gerätetest dazu (2026-10-25, vier Übernahmen): Der Cache
    wird jedes Mal neu geladen und hält danach die geholte Position, aber
    zwei Übernahmen öffneten trotzdem am Kapitelanfang (14:56 Kapitel 1 →
    2, 15:02 im Kapitel 2, beide kurz nach einem Neustart). Der Cache war
    also nicht die ganze Ursache. Gemeinsam ist den Fehlschlägen (auch
    13:47): Das Buch wurde vorher auf der Seite verlassen, auf der es
    geöffnet hatte, ohne zu blättern. Und die Prüfung ließ um 14:56 den
    Kapitelanfang als „übernommen“ durch (36 Schritte bei Grenze 40).
  - [x] **ich:** Grenze in `near()` auf 12 Schritte (die richtigen Seiten
    lagen 2 bis 4 davor), Toasts länger (5 s, Probleme 8 s), und das Log
    zeigt die Zeile beim Öffnen, die Spalten, die das Neuladen ändert, und
    die Spalten, die Nickel bis zum Verlassen ändert.
  - [x] **du:** Gezielter Gerätetest (2026-10-25, 15:14–15:32): Blättern
    ist es nicht, und keine Spalte der Zeile unterscheidet Erfolg und
    Fehlschlag (15:18 und 15:19 öffneten mit derselben Zeile). Seit dem
    Cache-Neuladen klappten alle 6 Übernahmen mit schon eingeschaltetem
    WLAN; von 5, bei denen das Plugin erst aufs WLAN wartete (lokale
    Ereignisschleife mitten in `setVolume`), schlugen 3 fehl.
  - [x] **ich:** Toast-Texte gekürzt: Der Tolino schneidet die erste Zeile
    nach etwa 24 Zeichen ab, die zweite nach etwa 45.
  - [x] **du:** Gerätetest WLAN aus / WLAN an (2026-10-25, 15:44–15:49):
    drei Übernahmen richtig, zwei davon mit Warten (7 s und 1 s); einmal
    kam kein WLAN (richtig behandelt). Stand mit Cache-Neuladen: ohne
    Warten 7 von 7 richtig, mit Warten 4 von 7. Nicht eindeutig.
  - [x] **ich:** Testschalter: Gibt es `.adds/NickelInkSync/debug-open-wait`,
    wartet das Plugin bei jedem Öffnen 10 s in einer lokalen
    Ereignisschleife, auch bei verbundenem WLAN.
  - [x] **du:** Gerätetest mit Schalter (2026-10-28, 15:06–15:22): Nickel
    öffnete in allen neun Übernahmen auf der Seite mit der gezogenen
    Stelle, mit und ohne Warten. Falsch war die Prüfung des Plugins: 3 s
    nach dem Öffnen kannte Nickel seine Stelle in acht von neun Fällen
    noch nicht (leer), und das Plugin hielt das für ein anderes Kapitel.
  - [x] **ich:** Prüfung nach dem Öffnen fragt bei leerer Stelle bis zu
    8 s lang jede Sekunde nach; bleibt sie leer, gilt die Übernahme nach
    dem Cache-Neuladen als geglückt (das nur in PR #26). Testschalter
    wieder ausgebaut.
  - [x] **du:** Kurzer Gerätetest der neuen Prüfung (2026-10-28,
    15:52–16:03): die Prüfung arbeitet, aber Nickel öffnete Book R danach
    immer am Anfang von Kapitel 1, auch nach einer Minute Pause.
  - [x] **ich:** Was sich geändert hat, liegt außerhalb des Plugins: Der
    Tolino hat sich am 28.10. selbst auf Firmware **5.18.279364**
    aktualisiert (vorher 5.18.264769). Auch ohne Plugin, nach einem
    `pull --write` über USB und Neustart, öffnet Nickel am Kapitelanfang.
    Spur, nicht bewiesen: Nickels Cache gezeichneter Seiten
    (`CachePersistenceRenderedPageData`); alle fehlgeschlagenen Stellen
    lagen in Seiten, die dort fehlten.
  - [x] **ich:** Vermutete Ursache (29.10.): Nickel nimmt die Stelle aus
    der Tabelle `tolino_bookmark`; dort stand für Book R der
    Kapitelanfang. Kobalt (MIT, develop, `tolino_progress.h`) schreibt die
    Zeile mit. PR #40 (Entwurf) schreibt sie jetzt auch.
  - [x] **du:** Gerätetest von PR #40 (2026-10-31): Beide Zeilen hatten
    die gezogene Stelle, Nickel öffnete trotzdem am Kapitelanfang. Damit
    ist `tolino_bookmark` nicht die Ursache; beide Stellen lagen wieder
    außerhalb der gespeicherten Seiten.
  - [x] **du + ich:** Ursache bewiesen (31.10., Syslog über NickelMenu):
    Nickel öffnet aus seinem Seiten-Cache und nimmt die erste gespeicherte
    Seite des Kapitels, wenn keine die Stelle enthält. Nur die Seiten des
    Zielkapitels löschen und neu starten: Book R öffnete bei R2-149, die
    Seiten von Kapitel 3 blieben. Auf dem Kobo 6.0 genauso.
  - [x] **ich:** CLI verwirft beim Schreiben die gespeicherten Seiten des
    Zielkapitels, wenn keine die Stelle enthält (USB und live); über USB
    meldet sie, dass das Gerät neu gestartet werden muss.
  - [x] **ich:** Der Desktop-Client sagt es auch: Nach „Holen“ mit
    verworfenen Seiten verlangt die Meldung, das Gerät auszuwerfen und neu
    zu starten (deutsch und englisch), sonst nur auszuwerfen.
  - [x] **du:** Gerätetest (31.10.): über USB 4 Seiten verworfen, nach
    Neustart R2-116 für R2-117; über das Plugin ohne Neustart 14 Seiten
    verworfen, R3-66 für R3-67.
  - [x] **ich:** Prüfung nach dem Öffnen wartet jetzt bis zu 15 s statt
    11 s (12 statt 8 Nachfragen, weiter jede Sekunde): Im Gerätetest vom
    31.10. kannte Nickel seine Stelle nach dem frischen Rendern erst nach
    8 s, ein größeres Buch bräuchte länger.
  - [ ] **du:** Gerätetest der längeren Prüfung: Toast, Stelle und im Log
    die Zeile „after opening (… ms)“.
  - [x] **ich:** Über USB blieb seit 5.18.279364 immer eine `-wal`-Datei
    liegen und blockierte das Schreiben; die CLI arbeitet sie jetzt nach
    einer Sicherung ein.
  - [x] **ich:** Neustart nach dem Schreiben über USB geprüft (Kobo-VM 6.0,
    2.11.): Nickel sucht seine Seiten bei jedem Öffnen in der Datenbank;
    Seiten verwerfen und auswerfen reicht, auch wenn das Buch in derselben
    Sitzung schon offen war. CLI, Desktop-Client und READMEs sagen jetzt
    „auswerfen“, den Neustart nur als Rückfall
    (`docs/nickel-6.0-symbols.md`, „Restart after a USB write“).
  - [x] **du:** Gerätetest ohne Neustart am Tolino (2.11., 5.18.279364,
    Book R EPUB, Plugin aus): Kapitel 2 ab Anfang gelesen, in KOReader
    R2-178, `pull --write` verwarf 27 Seiten, nur ausgeworfen: öffnete bei
    R2-178. (Ein erster Lauf mit R2-121 lag in einer gespeicherten Seite,
    nichts verworfen, ebenfalls richtig.)
  - [x] **du:** Derselbe Gerätetest am Clara BW (2.11., 6.0.279225, Plugin
    aus): Kapitel 2 ab Anfang gelesen, KOReader bei R2-178/179,
    `pull --write` schrieb EPUB und zugeordnetes kepub und verwarf 41 Seiten
    (Kapitelanfang dabei), nur ausgeworfen: das EPUB öffnete an der
    Stelle (Seite mit R2-179 oben, wie in KOReader).
  - [ ] **ich (falls das Plugin ohne Neustart nicht reicht):** die Seiten
    über Nickel selbst verwerfen (`PersistedCacheManager`); als letzter
    Ausweg `deleteAllRelatedToBookInfo`. Früherer Plan, falls auch das
    nicht hilft: Position über Nickel selbst
    schreiben, Vorbild Kobalt (MIT, nur das Konzept):
    `VolumeManager::updateBookmark`, `updateFields` in einer
    `DatabaseAdapter`-Transaktion; die CLI entscheidet dann nur noch
    (`pull --live --emit` ohne `--write`), die Tagessicherung bleibt vor
    jedem Schreiben.
  - [ ] **ich (nach dem Test):** Gerätesprache: Das Log zeigt `QLocale()`
    beim Start; folgt sie Nickels Spracheinstellung, kommen die Meldungen
    auch auf Englisch.
  - [x] **ich:** Aus den Gerätetests (25.–28.10.) übernommen: kürzere
    Toast-Texte (der Tolino schneidet nach etwa 24/45 Zeichen ab), längere
    Anzeige (5 s, Probleme 8 s), strengere Nähe-Prüfung nach dem Öffnen
    (12 statt 40 Knoten), die Prüfung fragt bei noch leerer Stelle bis zu
    8 s nach, und das Log zeigt die Buchzeile beim Öffnen und was Nickel
    bis zum Verlassen daran ändert. Das Neuladen von Nickels Buch-Cache
    bleibt in PR #26.
- [x] **N7 Kobalt-Schutz** (von dir gewünscht, 2026-10-31) · Kobalt
  schreibt die Lesepositionen selbst (über Nickel, auch auf dem Tolino) und
  hat auf `develop` seit dem 25.10. einen eigenen KOSync. Erkennung wie
  Kobalts eigene Deinstallation: `libkobalt.so` (oder `.failsafe`) da,
  `.adds/kobalt/uninstall` da, kein `uninstall-now`; über USB nur die
  beiden Dateien. Ein Ordner `.adds/kobalt` allein zählt nicht.
  - [x] **ich:** Regel in `src/kobalt.rs` (Tests mit Fake-Gerätebaum),
    CLI verweigert `--live` (`NICKELINKSYNC_ERROR kobalt`) und `--write`,
    `push` per USB warnt; D2 sperrt „Holen“ und zeigt einen Hinweis;
    Plugin reicht beide Hooks durch (Prüfung in `init` und bei jedem
    Öffnen und Verlassen, bis zum Neustart), Toast einmal je Erkennung
    (`kobalt-noticed`), Log bei jedem Start. Override
    `.adds/NickelInkSync/ignore-kobalt`.
  - [x] **du:** Erster Gerätetest (2026-10-31, 17:35–17:45): Erkennung
    richtig (installiert, `ignore-kobalt`, entfernt mit liegen gebliebenem
    Ordner). Der Toast kam nicht: Mit Kobalt als Standard-Leser ruft Nickel
    seinen eigenen Leser und damit `setVolume` nie auf. Beim allerersten
    Start nach der Installation fehlt Kobalts `uninstall` noch (erwartet).
  - [x] **ich:** Toast jetzt 20 s nach `init`, wenn Kobalt beim Start
    erkannt wird (weiterhin auch beim Öffnen in Nickels Leser).
  - [x] **du:** Zweiter Gerätetest (2026-10-31, 17:52–17:58): ohne Kobalt
    (nur der Ordner `.adds/kobalt` übrig) geholt und gesendet (82 %);
    erster Start nach der Kobalt-Installation wie erwartet nicht erkannt;
    beim nächsten Start erkannt, Toast „NickelInkSync ist aus“ nach etwa
    20 s; beim Start danach kein Toast mehr (`kobalt-noticed`).
- [x] **N6 Zuordnungen auf dem Gerät** · in **C3** (Abschnitt
  Kobo-Client) aufgegangen; Weg (a) gewählt, nie Original-EPUBs aufs Gerät ·
  Lücke, die du am 2026-10-29 bemerkt hast: Am Rechner sendet `push` ein
  zugeordnetes Buch (kepub, von Calibre verändertes EPUB) unter der
  Prüfsumme der Original-EPUB, mit der Stelle in sie übersetzt; KOReader
  findet sie, wenn er dieselbe Original-Datei hat. Das Plugin auf dem
  Gerät (`--live`) kennt aber keine Zuordnungen und sendet unter der
  eigenen Prüfsumme der Gerätedatei, mit der KOReader nichts anfangen
  kann; dasselbe beim Holen. Für die Übersetzung braucht die Bibliothek
  heute die Original-EPUB selbst, und die liegt nicht auf dem Gerät.
  - [x] **ich:** Weg wählen und vorschlagen: (a) beim Anlegen der
    Zuordnung eine kleine Übersetzungstabelle erzeugen (Prüfsumme des
    Originals, Kapitelpfade, Textlängen bzw. Offsets je Kapitel kepub ↔
    Original) und nach `.adds/NickelInkSync/` aufs Gerät legen (die GUI
    beim Kopieren in E5 oder per Knopf, die CLI per Befehl); (b) die
    Original-EPUB verborgen mit aufs Gerät kopieren (einfacher, kostet
    Speicher). Beide Richtungen, Kobo und Tolino.
  - [x] **ich:** Umsetzen in Bibliothek und CLI (`--live` liest die
    Tabelle), Tests mit Book A/R (kepub ↔ EPUB) und Calibre-artiger
    Umwandlung: C3.
  - [x] **du:** Am Gerät geprüft (2026-11-01, siehe C3).

## Kobo-Client

Client auf dem Gerät (ein Plugin für Kobo 6.0 und Tolino 5.18, dazu eine
Qt5-Fassung für Kobo 4.x), geplant
am 2026-10-31 (deine Antworten auf meine Rückfragen). **Teil von Release
0.1.0** (geändert am 2026-11-01; **C5 bis C7 seit 2026-11-07 in 0.1.1**); C0 bis C3 sind erledigt (PRs #66, #55,
#64, #67), C4 läuft. **Kobo 4.x** (seit 2026-11-01, Libra 2 mit
4.38.23852): C8 (Rust, PR #70) und C9 (Qt5-Erkundung und -Build) sind
erledigt, C10 (Plugin in Qt5) ist gebaut, der Gerätetest am Libra 2 steht aus. **Kobalt-Schutz (N7):** Jeder neue Einstieg des
Plugins (Öffnen, Verlassen, Buchmenü, NickelMenu-Auslöser, Nachholen der
Vormerkliste, Dialog) geht durch dieselbe Prüfung wie heute und bleibt
untätig, solange Kobalt installiert ist; Gerätetests mit Kobalt daneben
nur mit `.adds/NickelInkSync/ignore-kobalt`.
Fasst N2 und N6, die Kobo-Teile von K4 und P2 sowie O3/O4 (Rückfrage,
„Jetzt synchronisieren“) zusammen. Eine Etappe = eine eigene Session, ein
Branch von `origin/main`, ein PR. Jede Etappe endet mit dem PR, diesem
Abschnitt nachgezogen und einer Übergabenotiz; die nächste Session beginnt
mit diesem Abschnitt und der letzten Notiz. **C0, C1 und C2 können
gleichzeitig laufen** (andere Dateien, keine Abhängigkeit); ab C3 der Reihe
nach, mit einem Gerätetest dazwischen.

Festgelegt (2026-10-31):

- **Zuordnungen auf dem Gerät:** Der Rechner legt einen Bibliotheksindex
  (Titel, Autor, Prüfsumme der Originale) und je zugeordnetem Buch eine
  Übersetzungstabelle nach `.adds/NickelInkSync/`. **Nie** Original-EPUBs
  aufs Gerät. Ohne Index zeigt das Plugin eine Meldung und sendet nichts;
  gesendet wird nur ein Buch, dessen Prüfsumme im Index als Original steht
  oder das zugeordnet ist (auch auf dem Tolino). Holen bleibt erlaubt.
- **Dieselben Zuordnungen** in CLI, GUI und Gerät, abgeglichen beim Anschließen.
- **Basis-Stand je Buch** (zuletzt mit dem Server abgeglichen),
  Drei-Wege-Vergleich: Gerät unverändert → Server gewinnt (löst
  `DateLastRead` beim Öffnen); Server unverändert → Gerät gewinnt; beide
  geändert → Konflikt. `not-in-line.tsv` geht darin auf.
- **Konflikt:** Das Plugin fragt beim Öffnen per Dialog und zeigt beide
  Stände (Prozent, Kapitel, Zeit, Gerät): Server übernehmen, Gerät
  behalten, später.
- **Offline:** Ein ohne WLAN geöffnetes Buch, dessen Position sich bewegt
  hat, wird vorgemerkt, ebenso ein gescheiterter Push. Die Vormerkliste wird
  bei jedem WLAN nachgeholt (Öffnen, Schließen eines beliebigen Buchs,
  „Jetzt synchronisieren“). Die Wartezeit auf WLAN beim Öffnen wird kürzer.
- **Oberfläche:** eigener Qt-Dialog im Plugin; Einstieg über eine
  NickelMenu-Datei in `.adds/nm/` (falls NickelMenu installiert ist, auf
  6.0 nur der Qt6-Fork von nicoverbruggen) und über das Buchmenü.
- **Position schreiben wie Kobalt** (MIT): über Nickel
  (`DatabaseAdapter::beginTransaction`, `VolumeManager::getById`,
  `updateBookmark`, `updateFields`, `endTransaction`,
  `Repository::refreshCache<Volume>`), nicht mit der CLI in die Datenbank.
- **kepub bevorzugt, EPUB ebenfalls**; ein kepub braucht immer eine Zuordnung.

Heutiger Stand, am Code geprüft (2026-10-31): Das Plugin merkt sich keinen
gescheiterten Push (ohne WLAN nur ein Toast); ein ohne WLAN geöffnetes Buch
wird beim Schließen gar nicht gesendet; der Zustand liegt nur im Speicher.
Auf dem Kobo ist das Plugin inaktiv.

- [x] **C0 Erkundung in der Kobo-VM** (6.0.279225) · PR #66 · Ergebnis in
  [`docs/nickel-6.0-symbols.md`](docs/nickel-6.0-symbols.md)
  - [x] **ich:** Alle 23 Symbole des Plugins gibt es in 6.0 unverändert,
    ebenso Kobalts Schreibweg, Buchmenü (`setupBookListMenu`) und Dialog
    (`ConfirmationDialogFactory::getConfirmationDialog`); Signaturen per
    Disassembly geprüft. `updateBookmark` ändert nur das `Volume` im
    Speicher, `updateFields` schreibt und zieht Nickels `Volume`-Cache nach.
    In der VM (Wegwerf-Plugin, Book R als kepub und EPUB) öffnet Nickel an
    der geschriebenen Stelle, auch beim ersten Öffnen nach dem Start, aber
    **Kobalts Weg umgeht den Seiten-Cache nicht**: Ist der Anfang des
    Zielkapitels gespeichert und keine Seite hält die Stelle, öffnet Nickel
    beim ersten Öffnen nach dem Start am Kapitelanfang (kepub und EPUB).
    Werden die Seiten des Kapitels in derselben Transaktion verworfen (wie
    die CLI seit N5), öffnet es richtig, ohne Neustart. Buchmenü-Eintrag
    und Dialog erscheinen und reagieren; ein eigener `QPushButton` im Dialog
    bekommt keine Berührung. Kobalt-Schutz: die CLI verweigert in der
    Kobo-VM richtig; im Plugin endet `init()` auf dem Kobo vor der Prüfung.
    Die VM behält jetzt `/mnt/aux` (gezeichnete Seiten) über Neustarts.
- [x] **C1 kepub zurück** (K4-Rest): XPointer → `#kobo.N.M:offset`
  (UTF-16-Einheiten), Fixtures Book P/Q/R; `pull --write` am Kobo per USB.
  - [x] **ich:** Übersetzung, Tests (PR #55; siehe K4).
  - [x] **du:** Book R vom Server per USB auf den Kobo schreiben, auswerfen,
    öffnen: geprüft 2026-10-31, öffnet an der Stelle (siehe K4).
- [x] **C2 Basis-Stand und Vormerkliste** (nur Bibliothek und CLI) · PR #64
  - [x] **ich:** `.adds/NickelInkSync/state.json` (`src/state.rs`): je Buch
    Basis (Dokument, Nickels Stelle, XPointer, Prozent, Server-Zeit),
    `not_in_line` und `pending`; geschrieben nach `push`, `pull --write`
    und bei `IN_SYNC`, gemeinsam für `--live` und USB, atomar und nur mit
    den Büchern des eigenen Laufs (zwei Läufe auf dem Gerät können sich
    überschneiden). Drei-Wege-Regel (`sync::decide_since`, `Moved`): nur
    der Server bewegt → Server gewinnt (Nickels `DateLastRead` beim Öffnen
    zählt nicht mehr), nur das Gerät → Gerät gewinnt, auch rückwärts,
    beide → die bisherige Regel mit `confirm`; `always` fragt weiter;
    ohne Basis wie bisher. `not-in-line.tsv` wird beim Laden übernommen
    und beim ersten Speichern gelöscht (auch später neu geschriebene
    Zeilen des heutigen Plugins), gleiches Verhalten wie bisher.
    `push --live` merkt bei `timeout`/`network`/`server` vor
    (`NICKELINKSYNC_PENDING<tab>ID<tab>marked<tab>Art`); neu
    `sync --pending` (`--live`, `--emit`) mit einer Zeile je Buch und
    `NICKELINKSYNC_PENDING_LEFT`; neue Zeile `NICKELINKSYNC_CONFLICT` mit
    beiden Ständen für C5. Kobalt-Schutz greift auch hier. Tests:
    `tests/state.rs` (Mock-Server, auch der Übergang von
    `not-in-line.tsv`), `tests/kobalt.rs`. Format und Zeilen: Notiz
    `2026-11-01-kobo-c2-handover.md`.
  - [x] **ich (C4):** Plugin wertet `PENDING`/`CONFLICT` aus, ruft
    `sync --pending` bei jedem WLAN auf und schreibt `not_in_line` selbst
    in `state.json` statt in `not-in-line.tsv` (PR #72).
  - [x] **D2:** Der Desktop-Client schreibt die Basis bei „Holen“ (über
    `Session::write`) und jetzt auch beim Senden: nach einem gesendeten
    Update `Session::agreed_sent`, bei „gleich“ `Session::agreed`, am Ende
    `save_state`; auch nach einer beantworteten Rückfrage. Der Kindle
    führt keine Basis.
- [x] **C3 Zuordnungen auf dem Gerät** (ersetzt N6; braucht C1) · PR #67
  - [x] **ich:** `nickelinksync device prepare --device … [--library …]`
    (`src/prepare.rs`) schreibt nach `.adds/NickelInkSync/`: `library.json`
    (Index: Prüfsumme, Titel, Autoren je Original), `mappings.json` (die
    Zuordnungen der Bücher auf dem Gerät, dazu `added`/`removed` für am
    Gerät angelegte bzw. entfernte, die prepare über die Prüfsumme in der
    Bibliothek in den Store übernimmt) und je zugeordnetem Buch
    `tables/<Geräte-MD5>.json` (`src/table.rs`: Textknoten mit
    KOReader-Pfad, Absatz- und Satzanfängen, keine Buchtexte; nie eine
    EPUB). `--live` liest sie: zugeordnet mit Tabelle → unter der
    Original-Prüfsumme, gesendeter Absatz-/Satzanfang genau wie am Rechner
    (`tests/table.rs`: Book A, Book R, Calibre-artige Umwandlung, geänderter
    Text; `tests/prepare.rs`); `exact` und der Rückweg zwischen zwei Ankern
    anteilig. Ohne `library.json` senden `push --live` und
    `sync --pending --live` nichts (`NICKELINKSYNC_NO_INDEX<tab>Meldung`);
    gesendet wird nur ein Original aus dem Index oder ein Buch mit
    Zuordnung und Tabelle (sonst `NOT_SENT not-mapped` bzw. `no-table`).
    Holen bleibt erlaubt. Kobalt-Schutz unverändert. Ein kepub, das
    `copy` auf ein vorbereitetes Gerät kopiert, bekommt Zuordnung und
    Tabelle sofort (`prepare::put_mapping`). Format und Zeilen: Notiz
    `2026-11-01-kobo-c3-handover.md`.
  - [x] **du:** Am Kobo geprüft (2026-11-01, Clara BW 6.0): kepub mit
    Calibre übertragen, der EPUB zugeordnet, `device prepare`, am Kobo
    gelesen, Fortschritt gesendet; KOReader lädt ihn in der Original-EPUB.
    Alle Schritte der Anleitung in PR #67 erfolgreich.
- [x] **C4 Plugin auf dem Kobo** (ersetzt N2; braucht C0, C2, C3) · PR #72
  - [x] **ich:** Ein Plugin, ein `Kobo.tgz` für Kobo 6.0 und Tolino 5.18;
    auf dem Kobo aktiv (die Tolino-Sperre in `init()` ist weg, inaktiv nur
    ohne CLI, `config.ini` oder Nickels Schreibweg), Kobalt-Schutz in
    jedem Einstieg. **Öffnen (Kobo):** neu `pull --live --nickel-writes
    --emit`: die CLI entscheidet nur und macht die Tagessicherung, dann
    `NICKELINKSYNC_LAST_READ`, `NICKELINKSYNC_STALE_PAGES` (die Seiten,
    die `drop_stale_pages` verwerfen würde) und `NICKELINKSYNC_BASE`
    (JSON); nichts in Datenbank oder `state.json`. Das Plugin schreibt in
    einer Nickel-Transaktion (`getById`, `updateBookmark`,
    `setPercentRead`, `updateFields` mit den geänderten Spalten), verwirft
    die genannten Seiten auf Nickels Verbindung, bei Fehler Rollback und
    `refreshCache<Volume>`, gibt Nickel ein frisches `getById`-Volume und
    setzt erst danach die Basis in `state.json` (gleiche Atomik wie die
    CLI). Tolino unverändert über `--write` bis C7. **Schließen:** Push
    nach höchstens 15 s WLAN-Warten (kein WLAN-Dialog mehr), scheitert
    er, merkt die CLI vor („Fortschritt vorgemerkt“); ein ohne Server
    geöffnetes Buch nur, wenn die Seite beim Verlassen eine andere ist
    als beim Öffnen. Vormerkliste (`sync --pending`) im Hintergrund,
    sobald der Server antwortet (5 s nach dem Öffnen, nach jedem Push),
    nur wenn `state.json` vorgemerkte Bücher hat. WLAN-Wartezeit beim
    Öffnen 3 s. `CONFLICT` nur gemeldet, Buch bleibt liegen und wird
    nicht gesendet. `NO_INDEX`: einmal pro Start „Gerät nicht
    vorbereitet“. `not_in_line` schreibt das Plugin jetzt in
    `state.json`. CI prüft die Symbole gegen 5.18 und 6.0 (die 6.0-Liste
    macht `tools/libnickel_symbols.sh` aus Kobos Update, gecacht); das
    CI-Artefakt heißt jetzt `NickelInkSync-device`. Test:
    `tests/write.rs` (`--nickel-writes`). Kobo-VM (Book „C4 Alpha“ u. a.,
    `NICKEL_VM_INSTANCE` für eine zweite VM neben einer anderen
    Session): Server-Stand beim ersten Öffnen nach dem Start (5 Seiten
    verworfen, öffnet auf der Seite), zweites Öffnen im selben Lauf,
    offline lesen → vorgemerkt, offline öffnen ohne Lesen → nichts,
    anderes Buch öffnen → nachgeholt (2 Bücher), Konflikt, ohne
    `device prepare`, Kobalt-Schutz. Code bleibt Qt5/C++11-tauglich
    (für C9/C10).
  - [x] **du:** Am Clara BW (2026-11-02): Book R (kepub, zugeordnet) mit
    WLAN geöffnet, öffnet bei R2-40 für KOReaders R2-41 („Fortschritt
    übernommen“); WLAN aus, bis R2-64 gelesen, geschlossen („Fortschritt
    vorgemerkt“); Foundation (kepub) offline erst „Buch nicht zugeordnet“,
    nach Zuordnen und `device prepare` offline gelesen bis Kapitel 5,
    vorgemerkt; WLAN an, Book R geöffnet: „Vorgemerktes gesendet / 2
    Bücher“. KOReader: Book R bei R2-63, Foundation auf der Seite mit dem
    Absatzanfang (Kapitel 5, 183 Zeichen vor der Kobo-Stelle,
    `position = paragraph`). Vorher lag auf dem Kobo das alte Paket vom
    25.10. (`dist/` war nicht neu gebaut), das Plugin blieb inaktiv.
  - [x] **du:** Der Fall, den die VM nur annähert (2026-11-02): Foundation
    am Anfang von Kapitel 7 verlassen, in KOReader weiter in Kapitel 7,
    Kobo neu gestartet, geöffnet: richtig auf der Seite mit KOReaders
    Stelle (`kobo.15.8:0` für `kobo.15.9:27`), beim Verlassen „Fortschritt
    ist aktuell“. Kein Seitenverwerfen nötig: Nickel hatte das ganze
    Kapitel schon gerendert, eine Seite hielt die Stelle. Das Verwerfen
    selbst ist in der Kobo-VM geprüft (5 Seiten, richtig geöffnet).
- [ ] **C5 Dialog bei Konflikten** → nach [Release 0.1.1](#release-011) verschoben (2026-11-07).
- [ ] **C6 Oberfläche** → nach [Release 0.1.1](#release-011) verschoben (2026-11-07).
- [x] **C8 Kobo 4.x lesen und zurückschreiben** (Rust, CLI und GUI; kann
  sofort parallel laufen) · neues Testgerät **Kobo Libra 2, Firmware
  4.38.23852** (Qt5, von dir am 2026-11-01).
  - [x] **du:** Datenbankkopie des Libra 2 (`.kobo/KoboReader.sqlite`, Nickel
    beendet bzw. per USB) mit Book P/Q/R als kepub und EPUB, P und Q markiert
    wie zuvor, für neue Fixtures (`tools/make_db_fixture.py`).
  - [x] **ich:** Rückrichtung für Kobo 4.x (`from_xpointer`,
    `from_xpointer_kepub` mit `PointFlavor::JoinText`): Adobe-Point, in dem
    Text, CDATA und Kommentare eines Elements ein Knoten sind (Offset in
    UTF-8-Bytes durch Text und CDATA, Kommentare zählen nicht), und
    kepub-Span ohne Offset (`#kobo.N.M`; die Stelle im Span geht verloren,
    Nickel öffnet am Span-Anfang). Rundreise mit allen 16 EPUB-Markierungen
    des Clara BW 4.46 und KOReaders Markierungen derselben Wörter (p7w03
    nach dem Kommentar `/1/4/16/1:13`, q5w04 nach CDATA `/1/4/12/1:20`),
    KOReaders kepub-Markierungen ergeben die Kobo-Spans, Book A und Book R
    als Leseposition (`tests/xpointer.rs`). `pull --write` und „Holen“ im
    Desktop-Client schreiben beides auf 4.x (`tests/write.rs`,
    `tests/sync_mapping.rs`, `gui/src/sync.rs`); Sicherung vorher und
    Kobalt-Schutz wie bisher. Modell-ID 388 = Kobo Libra 2. READMEs,
    `SCOPE.md`, `ROADMAP.md` nachgezogen.
  - [x] **ich:** Am Libra 2 gelesen (2026-11-02, nur lesend): `.kobo/version`
    meldet 4.38.23852 und Modell-ID 388 (Kobo Libra 2), Datenbankversion
    174. **Kein Seiten-Cache** auf 4.38: keine `CachePersistence*`-Tabellen,
    die CLI verwirft dort also nichts (wie für ältere Firmware vorgesehen).
  - [x] **ich:** 4.38-Fixtures (`kobo-libra2-4.38-probe-books.sql`,
    2026-11-02): alle 32 Markierungen beim ersten Versuch gespeichert und
    genau wie auf 4.46; Book R im EPUB `chapter2.xhtml#point(/1/4/104/1:113)`
    (R2-51), im kepub `chapter3.xhtml#kobo.122.1` (R3-121), beide hin und
    zurück (`tests/xpointer.rs`). In den kepubs zeigt 4.38 wie 4.46 zwei
    Markierungen verschoben an (p3w03 nach `&amp;`, q7w03 nach `&lt;`/`&gt;`),
    gespeichert sind sie richtig.
  - [x] **du:** Book R vom Server per USB auf den **Libra 2** schreiben
    (EPUB und zugeordnetes kepub), auswerfen, öffnen (2026-11-02): KOReader
    in R3-161 (Zeichen 99), `pull --write` schrieb ins EPUB
    `chapter3.xhtml#point(/1/4/324/1:102)` und ins kepub
    `chapter3.xhtml#kobo.162.1`, mit Sicherung. Das EPUB öffnete bei R3-160
    (sehr große Schrift), Nickel speicherte danach genau den geschriebenen
    Point zurück; das kepub öffnete bei R3-158, Nickel speicherte
    `kobo.160.1` (R3-159) als Seitenanfang: die Seite mit dem Anfang von
    R3-161, wie auf 4.x erwartet (nur der Span).
- [x] **C9 Qt5-Erkundung für Kobo 4.x** (ersetzt N1) · Ergebnis in
  [`docs/nickel-4.38-symbols.md`](docs/nickel-4.38-symbols.md)
  - [x] **du:** Download des Firmware-Updates 4.38.23852 für den Libra 2
    erlaubt (2026-11-01; `kobo-update-4.38.23852.zip`, 121 MB, von Kobos
    Update-Server).
  - [x] **ich:** 18 der 23 Plugin-Symbole gibt es auf 4.38 unverändert;
    es fehlen `Content::getId(QString const&)` (4.x: `getId()` ohne
    Argument), `Bookmark::isNull`, der kopierende `Volume`-Konstruktor,
    `Bookmark::getStartLocation` und `ContentLocation` (die Klasse gibt es
    auf 4.x nicht); Ersatz für C10 im Dokument. `Volume` und `Bookmark`
    sind 8 Bytes (6.0: 12), `Device::isTolino` ist fest `false`. Kobalts
    Qt5-Schreibweg geht mit drei Unterschieden zu 6.0 (`dbConnection` nimmt
    `QString` als Wert, `updateBookmark` ohne letztes `int`, kein
    `ContentLocation::create`); Buchmenü (`setupBookListMenu` ohne `bool`),
    Dialog, Toast und WLAN sind da. Qt5-Build aus derselben Quelle
    (`#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)`, NickelTC, GCC 4.9):
    `just plugin-qt5` → `plugin/KoboRoot.tgz` (4.x-Layout); auf 4.x prüft
    `init()` Kobalt, meldet im Log, dass es auf 4.x läuft, und bleibt
    untätig, bis C10 das Verhalten bringt. CI-Schritt im Job `check`:
    Symbole gegen `tools/libnickel-4.38.23852.syms.gz` (die genutzten
    Klassen), Build, ARM, Qt 5, Handler, Layout; Artefakt
    `NickelInkSync-kobo4-KoboRoot`. Jedes Symbol des gebauten Plugins
    löst gegen libnickel, Qt und libc von 4.38 auf. Auf 4.x gibt es
    NickelMenu upstream (pgaskin, Qt5) für C6. Nach dem Merge von #62
    (2026-11-02) baut der Release-Workflow die Qt5-Fassung mit und legt
    sie als `kobo-4.x/KoboRoot.tgz` ins Gerätepaket (eigener Ordner,
    damit klar ist, welche Datei wofür ist; 6.0 wendet laut
    `/etc/init.d/ota` nur `Kobo.tgz` an); Release-Notes und READMEs sagen,
    welche Datei für welche Firmware ist.
  - [x] **du:** Am Libra 2 geprüft (2026-11-02, 4.38.23852): `KoboRoot.tgz`
    installiert, das Plugin lädt und meldet im Log `inactive: Kobo firmware
    4.x (Qt 5.2.1), plugin loaded; syncing on 4.x follows later`; Öffnen und
    Schließen eines Buchs ändern nichts.
- [x] **C10 Plugin auf Kobo 4.x** (braucht C4, C8, C9): dasselbe Verhalten
  wie C4 in der Qt5-Fassung aus derselben Quelle (`#if` für Qt5/Qt6, wo
  nötig). Ab C10 bauen C5 und C6 beide Fassungen.
  - [x] **ich:** Die Qt5-Fassung (`just plugin-qt5` → `KoboRoot.tgz`) ist
    auf 4.x aktiv und macht dasselbe wie C4 (Öffnen, Schließen,
    Vormerkliste, Toasts, `NO_INDEX`, `not_in_line`, Kobalt-Schutz in
    jedem Einstieg). Unterschiede nur in `#if QT_VERSION`-Blöcken und
    kleinen Adaptern, die Qt6-Fassung verhält sich unverändert:
    `Bookmark::isNull` → Vergleich mit einem leeren `Bookmark`
    (`operator==`); Nickels Stelle aus `Bookmark::getStartContainerPath()`
    statt `ContentLocation` (der kepub-Leser füllt sie mit genau dem
    String, den `saveBookmark` als `ChapterIDBookmarked` speichert, laut
    Disassembly von `KepubBookReaderBase::initializeBookmark`; ohne
    Kapiteldatei kommt sie aus `getChapterFilePath()`, die erste Stelle
    steht im Log); Schreibweg mit `dbConnection(QString, bool)`,
    `updateBookmark` ohne letztes `int`, Position als String ohne
    `ContentLocation::create`; kein Seitenverwerfen (4.38 hat keinen
    Seiten-Cache, die CLI nennt keine Seiten). Die CLI liefert mit `pull
    --live --nickel-writes --emit` auf 4.x dieselben Strings wie `pull
    --write` (kepub-Span ohne Offset, EPUB-Point mit zusammengefasstem
    Text; `tests/sync_mapping.rs::live_pull_hands_nickel_the_kobo_4_forms`).
    Jedes Symbol der Qt5-Bibliothek löst gegen libnickel, Qt 5.2.1 (auch
    QtSql), libstdc++ und libc von 4.38 auf; Symbolprüfung gegen 4.38,
    5.18 und 6.0 in der CI (unverändert, die Referenz deckt die neuen
    Symbole ab). Nicht ohne Gerät prüfbar: ob die Stelle aus dem
    `Bookmark` beim EPUB (Adobe-Leser) dieselbe Form hat (das Log zeigt
    es beim ersten Öffnen).
  - [x] **ich:** Erster Gerätetest am Libra 2 (2026-11-02): Plugin aktiv,
    beim kepub liest es Nickels Stelle richtig
    (`OEBPS/text/chapter3.xhtml#kobo.160.1`), aber Öffnen lief am Plugin
    vorbei (keine Toasts, kein Pull). Ursache laut Disassembly und Code:
    NickelStorygraph hookt auf dem Libra 2 ebenfalls
    `ReadingController::setVolume` und lädt nach uns; bis 0.1.x mit dem
    alten NickelHook (pgaskin), das einen vorhandenen Hook nicht
    weiterreicht, und in jeder Version ruft es statt seines Originals
    NickelHardcovers Hook auf, wenn der geladen ist (auch auf 5.18/6.x).
    Das Plugin setzt seinen Öffnen-Hook nach dem Laden aller Mods (und 20 s
    später noch einmal) wieder davor, der fremde Hook läuft danach als
    Original (`keep_open_hook()`, Log: „opening hook: another mod …“). Ruft
    ein verkettender Mod (NickelStorygraph ab 0.2.0) uns von dort zurück,
    geht dieser Aufruf direkt an Nickels `setVolume` (`g_in_original`),
    sonst entstünde eine Endlosschleife (Befund der Session
    „Hook-Konflikt NickelStorygraph auf 5.18/6.x prüfen“).
  - [x] **du/ich:** Zweiter Gerätetest am Libra 2 (2026-11-02, kepub):
    Öffnen übernimmt den Server-Stand (R1-27, „Fortschritt übernommen“),
    nicht zugeordnete Bücher melden „Buch nicht zugeordnet“. Aber offline
    gelesen und geschlossen kam keine Meldung und nichts wurde vorgemerkt;
    gesendet wurde erst, als später WLAN da war. Ursache: Qt 5.2 ruft ein
    verbundenes Lambda über eine Kopie auf, der Zähler im `mutable`-Lambda
    von `when_online()` blieb 0, das Warten endete nie. Jetzt mit fester
    Startzeit statt Zähler. Der „Konflikt“ blieb aus, weil `config.ini`
    dort `confirm = never` hat (dann gewinnt der neuere Stand, kein
    Konflikt).
  - [x] **du:** Dritter Gerätetest am Libra 2 (2026-11-02, kepub, mit
    `confirm = backwards`): offline bis R2-162 gelesen → „Fortschritt
    vorgemerkt“ (nach 15 s); Book P offline geöffnet und geschlossen →
    nichts gesendet; WLAN an, Book Q geöffnet → „Vorgemerktes gesendet / 1
    Buch“; KOReader R2-130, Libra 2 offline R2-197 → „Konflikt, nicht
    geladen“, beim Verlassen „Nicht gesendet“. Ohne `library.json` meldet
    das Plugin beim Öffnen „Buch nicht zugeordnet“ statt „Gerät nicht
    vorbereitet“: `pull` meldete ohne Index `not-mapped`, `NO_INDEX` kam
    nur von `push`, und der wurde danach nicht mehr versucht (wie bei C4).
    Behoben (von dir gewünscht, in PR #77): `pull --live --emit` meldet
    ohne `library.json` zuerst `NICKELINKSYNC_NO_INDEX` (Holen bleibt
    erlaubt, `tests/prepare.rs`), das Plugin nennt dann beim Öffnen „Gerät
    nicht vorbereitet“ (einmal pro Start mit dem Hinweis „Am Rechner
    ‚Gerät vorbereiten‘“), auf Kobo 6.0 und Tolino ebenso. Am Gerät noch
    nicht geprüft.
  - [x] **du:** Dieselben Tests mit dem EPUB von Book R am **Libra 2**
    (2026-11-02): KOReader R3-10 → Libra 2 öffnet bei R3-10 („Fortschritt
    übernommen“, `chapter3.xhtml#point(/1/4/20/1:208)` geschrieben, Nickel
    meldet genau diese Stelle zurück, die Stelle aus dem `Bookmark` hat
    also auch beim Adobe-Leser die Form von `ChapterIDBookmarked`); offline
    bis R3-51 → „Fortschritt vorgemerkt“; WLAN an, anderes Buch →
    „Vorgemerktes gesendet / 1 Buch“; Libra 2 offline R3-95, KOReader R3-72
    → „Konflikt, nicht geladen“. Die Meldung „Fortschritt vorgemerkt“ kam
    einmal erst beim Öffnen von Book P: das Warten auf WLAN dauerte dort
    35 s statt 15 (Nickel war beschäftigt) und der Push ohne DNS noch 8 s,
    die Meldung gehörte zu Book R.
  - [x] **du:** Kobalt-Schutz am **Libra 2**: Kobalt (Qt5) installieren,
    ohne `.adds/NickelInkSync/ignore-kobalt`; Öffnen und Schließen ändern
    nichts, das Log meldet `inactive: Kobalt is installed`, die Meldung
    „NickelInkSync ist aus“ kommt einmal. Geprüft am 2026-11-07
    (`docs/device-tests/kobo-libra-2-4.38.md`), ebenso am Clara BW.
  - [x] **du:** Ohne `library.json` öffnen: „Gerät nicht vorbereitet“
    statt „Buch nicht zugeordnet“. Geprüft am 2026-11-07 am Libra 2 und am
    Clara BW; beim Schließen „Nicht gesendet“.
- [ ] **C7 Tolino** → nach [Release 0.1.1](#release-011) verschoben (2026-11-07).
- [x] **(in D2, nach C3)** Knopf „Gerät vorbereiten“ in der GUI
  (`prepare::prepare` mit Fortschritt, `Prepared::load` für den Hinweis);
  beim Kopieren (E5) schreibt `copy::copy` Zuordnung und Tabelle schon
  selbst (`prepare::put_mapping`), wenn das Gerät vorbereitet ist.
  Umgesetzt: Knopf neben „Aus Bibliothek hinzufügen …“ bei Kobo und
  Tolino, Fortschritt „n von m“, Meldung mit Originalen, Zuordnungen,
  Tabellen, übernommenen und entfernten Zuordnungen und allem, was nicht
  ging; darunter „Vorbereitet: <Zeit>“ oder der Hinweis „Gerät nicht
  vorbereitet“.
  - [x] **du:** „Gerät vorbereiten“ am Tolino oder Kobo (2026-11-01):
    Vorbereiten ist einfach, der Zeitpunkt der letzten Vorbereitung gut
    lesbar.

## Optional (später)

- [x] **O1 Manuelle Zuordnung** von Büchern (kepub ↔ EPUB), Testfall Heat 2.
  **Geht in D1 auf** (CLI mit manueller Buchzuordnung, höchste Priorität).
  Vorbild: BookOrbit, nur als Konzept (AGPL-3.0). Blockiert auch den
  Kindle-Sync (L1): Nichts im KFX führt zur EPUB-Prüfsumme; Kandidaten für
  den Schlüssel sind `content_id` (32 Hex-Ziffern, auch im Dateinamen),
  `title` (= Dateiname beim Senden) und `author`.
- [x] **O2 Normales EPUB auf Kobo 4.x** · **du:** ein normales EPUB auf den Clara
  geladen und geöffnet. Ergebnis: wie auf dem Tolino `#point(...)`,
  Kapitelzeilen sind ToC-Einträge (`SCOPE.md`).
- [ ] **O3 Einstellungsdialog** auf dem Gerät (der Dialog aus C6 ist der Rahmen dafür).
- [ ] **O4 Sync-Status** im Buchmenü; „Jetzt synchronisieren“ kommt mit C6.
- [~] **O5 Geräteübersicht**: alle Positionen eines Buchs nebeneinander; im
  Desktop-Client erledigt („Übersicht“), in `status` noch offen.
- [ ] **O6 Markierungen auf Kobo 5.x/6.x nachtesten** · nicht mehr
  blockiert: Der Clara BW ist seit 2026-10-28 auf 6.0. · **du:** Book P auf
  dem Kobo mit Firmware 6.0 markieren. Auf 4.46 zeigt Nickel Markierungen im kepub verschoben und im
  EPUB teils gar nicht an (gespeichert wird richtig); auf dem Tolino mit 5.18
  tritt das nicht auf.

## Langfristig (andere Gerätefamilien)

- [~] **L1 Kindle ↔ KOSync** · Machbarkeit in `SCOPE.md` (Abschnitt Kindle)
  - [x] **du:** Book A, C, P, Q und R per Send-to-Kindle auf den Paperwhite
    11 (5.19.2) geladen, in Book P und Q die Zielwörter markiert, in Book R
    zwei Seiten gelesen (R2-99, R3-85) und die Wörter oben notiert.
  - [x] **ich:** Nur lesend kopiert und ausgewertet: KFX ohne DRM, Position
    `lpr` in der `.yjf` per USB, `eid` + Offset in Codepoints, zeichengenau;
    Abweichungen zu KOReader bei Leerraum am Absatzanfang, Inline-Elementen
    und Kommentaren, CDATA-Text geht bei der Umwandlung verloren. Kein Weg
    vom KFX zur Prüfsumme der EPUB.
  - [x] **ich:** Anonymisierte Kindle-Fixtures für Book P, Q und R
    (`tools/make_kindle_fixture.py`, `tests/fixtures/kindle/`).
  - [x] **du:** Fixtures durchgesehen und freigegeben (2026-10-24).
  - [x] **du:** Festgelegt: Nutzer haben standardmäßig **keinen Jailbreak**.
    Der Funktionsumfang wird getrennt: ohne Jailbreak Desktop-CLI per USB
    (`.yjf`), mit Jailbreak zusätzlich `cc.db` und Sync ohne Kabel
    (Daemon auf dem Gerät). Nichts aus dem Grundumfang darf den Jailbreak
    brauchen.
  - [x] **ich:** KFX-Leser und Übersetzung `eid` + Offset → XPointer
    (`src/ion.rs`, `src/krds.rs`, `src/kfx.rs`, `src/kindle.rs`, eigener
    Ion- und KRDS-Leser): `lpr`, `fpr`, `page.history.record` und
    Markierungen aus `.yjf`/`.yjr`, Text und Positionskarten aus dem KFX;
    jede gespeicherte `pid` passt zu den Karten. Alle Markierungsanfänge in
    Book P und Q und die Book-R-Positionen ergeben KOReaders XPointer;
    nur q5w04 (CDATA) ist nicht exakt (Text weicht ab, gefunden über den
    Text danach).
  - [x] **ich:** Kindle in die CLI: `status`/`push` über die Zuordnung,
    nur lesend (D1 Etappe 4).
  - [ ] **du:** Prüfen, ob der Kindle eine geänderte `.yjf` übernimmt
    (Flugmodus, damit Amazons Sync nicht dazwischenfunkt).
