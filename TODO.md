# Arbeitsliste NickelInkSync

Konkrete Schritte aus der [Roadmap](ROADMAP.md), in der Reihenfolge, in der wir
sie angehen. Ein Branch und ein PR pro Punkt (oder pro zusammengehörender
Gruppe), ein Commit pro erledigtem Schritt. Nach einer Unterbrechung: diese
Datei und `git status` / `git log` lesen, dann beim ersten offenen Punkt
weitermachen.

Status: `[ ]` offen · `[~]` in Arbeit · `[x]` erledigt · `[!]` blockiert (Grund steht dabei)

Wer: **du** = braucht dich (Gerät, Entscheidung, Zugang) · **ich** = kann ich allein erledigen

Erledigte Punkte wandern in der Roadmap nach „Recently completed“.

**Reihenfolge der Geräte:** Erst wird alles, was mit dem **Tolino Shine Color
(5.18)** allein geht, umgesetzt und am Gerät getestet: P3, K4 rückwärts für
normale EPUBs, P2 am Tolino, N3, N4. Danach kommt der **Kobo** (Clara BW)
in einem eigenen Durchgang: P1-Durchlauf mit allen Geräten,
K4 rückwärts für kepub (`#kobo.N.M`), P2 am Kobo, N2. So geraten die
beiden Geräte beim Testen nicht durcheinander.

**Kobo jetzt auf Firmware 6.0:** Der Referenz-Clara-BW hat sich am
2026-10-28 selbst auf **6.0.279225** (Datenbankversion 222) aktualisiert.
Ein Kobo mit 4.x ist nicht mehr da. Der Kobo-Durchgang läuft deshalb auf
6.0 (Qt6 wie der Tolino, die Nickel-VM nutzt ebenfalls Kobo 6.0); was nur
4.x betrifft (Adobe-Point mit zusammengefasstem Text, Qt5-Plugin), bleibt
im Code, ist aber nur noch über die 4.46-Fixtures geprüft.

**Danach zuerst D1, dann D2:** Sobald der Tolino-Durchgang abgeschlossen
ist, kommt die **manuelle Buchzuordnung in der CLI (D1)**, danach der
**Client mit grafischer Oberfläche (D2)**, mit höchster Priorität und vor
allem anderen (auch vor dem Kobo-Durchgang). Ohne die Zuordnung gibt es
weder den Sync kepub ↔ EPUB noch den Kindle-Sync. Wartung und Fehlerbehebung
dürfen jederzeit dazwischengeschoben werden. **Stand 2026-10-29:** D1 ist
fertig, D2 läuft (E0 bis E5b und O5 erledigt; als Nächstes E6). **Danach
der Kobo-Client** (Abschnitt Kobo-Client, C0–C7, geplant am 2026-10-31).

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
  - [ ] **ich:** **E6** macOS: `.app` als `.dmg` aus der CI (macOS-Runner,
    eigener Job, kein Pflicht-Check), kepubify-Binaries (feste Version,
    SHA-256 gegen das Release geprüft) mit Lizenzhinweis in „Über“ und
    neben dem Binary, Release-Workflow per Tag, Hinweis auf neue
    Releases. Ohne Apple Developer ID nicht notarisiert: Seit macOS 15
    öffnet Rechtsklick → „Öffnen“ die App nicht mehr, sondern
    Systemeinstellungen → Datenschutz & Sicherheit → „Dennoch öffnen“
    (README).
  - [ ] **du:** Icon-Entwurf (SVG) ansehen; `.dmg` aus der CI installieren.
  - [ ] **ich:** **E7** Windows: Pfade für Konfiguration und Daten
    (`%APPDATA%`, heute nur `HOME`/XDG), Wechsellaufwerke erkennen,
    Anmeldeinformationsverwaltung, ZIP aus der CI (Windows-Runner); ohne
    Code-Signing warnt SmartScreen beim ersten Start.
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
    wird nicht geschrieben, kein 4.x-Gerät mehr.
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

- [!] **N1 Symbole für Kobo 4.46 (Qt5)** · blockiert: kein Kobo mit 4.x
  mehr (Clara BW seit 2026-10-28 auf 6.0). Nur noch, falls ein 4.x-Gerät
  zum Testen auftaucht.
  - [ ] **du:** Firmware-Update 4.46 für den Clara BW besorgen (wegen `libnickel.so`).
  - [ ] **ich:** Symbole für „Buch geöffnet“, „Buch geschlossen“ und „an
    Position springen“ suchen.
- [ ] **N2 Plugin für den Kobo 6.0** (statt Qt5 für 4.x) · geht in **C4**
  (Abschnitt Kobo-Client) auf
  - [ ] **ich:** Symbole aus `libnickel.so` von Kobo 6.0 prüfen (Vorbild
    N3, Tolino 5.18) und das Tolino-Plugin (Qt6) für den Kobo bauen;
    Paket `Kobo.tgz`.
  - [ ] **du:** Test auf dem Clara BW (6.0).
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
- [ ] **N6 Zuordnungen auf dem Gerät** · geht in **C3** (Abschnitt
  Kobo-Client) auf; Weg (a) gewählt, nie Original-EPUBs aufs Gerät ·
  Lücke, die du am 2026-10-29 bemerkt hast: Am Rechner sendet `push` ein
  zugeordnetes Buch (kepub, von Calibre verändertes EPUB) unter der
  Prüfsumme der Original-EPUB, mit der Stelle in sie übersetzt; KOReader
  findet sie, wenn er dieselbe Original-Datei hat. Das Plugin auf dem
  Gerät (`--live`) kennt aber keine Zuordnungen und sendet unter der
  eigenen Prüfsumme der Gerätedatei, mit der KOReader nichts anfangen
  kann; dasselbe beim Holen. Für die Übersetzung braucht die Bibliothek
  heute die Original-EPUB selbst, und die liegt nicht auf dem Gerät.
  - [ ] **ich:** Weg wählen und vorschlagen: (a) beim Anlegen der
    Zuordnung eine kleine Übersetzungstabelle erzeugen (Prüfsumme des
    Originals, Kapitelpfade, Textlängen bzw. Offsets je Kapitel kepub ↔
    Original) und nach `.adds/NickelInkSync/` aufs Gerät legen (die GUI
    beim Kopieren in E5 oder per Knopf, die CLI per Befehl); (b) die
    Original-EPUB verborgen mit aufs Gerät kopieren (einfacher, kostet
    Speicher). Beide Richtungen, Kobo und Tolino.
  - [ ] **ich:** Umsetzen in Bibliothek und CLI (`--live` liest die
    Tabelle), Tests mit Book A/R (kepub ↔ EPUB) und Calibre-artiger
    Umwandlung.
  - [ ] **du:** Am Gerät prüfen: ein Calibre-kepub auf dem Kobo bzw. ein
    Calibre-EPUB auf dem Tolino lesen, Buch verlassen, in KOReader mit der
    Original-EPUB öffnen.

## Kobo-Client

Client auf dem Gerät (ein Plugin für Kobo 6.0 und Tolino 5.18), geplant
am 2026-10-31 (deine Antworten auf meine Rückfragen). **Beginnt nach D2.**
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

- [ ] **C0 Erkundung in der Kobo-VM** (6.0.279225)
  - [ ] **ich:** Symbole des Plugins und Kobalts Schreibweg gegen
    `libnickel.so` von 6.0 prüfen (Signaturen per Disassembly); den
    Schreibweg als Wegwerf-Plugin in der VM ausprobieren (Buch öffnet an
    der geschriebenen Stelle, auch beim ersten Öffnen nach dem Start).
    Nickel öffnet auch auf dem Kobo 6.0 aus seinem Cache gerenderter
    Seiten (N5, 31.10.): prüfen, ob Kobalts Weg ihn umgeht oder die Seiten
    des Kapitels weiter verworfen werden müssen (wie die CLI seit N5);
    Ergebnis in `docs/nickel-6.0-symbols.md`. Zusätzlich die Symbole für
    einen eigenen Dialog und den Buchmenü-Eintrag
    (`ActionProxyMixin::setupBookListMenu`).
- [x] **C1 kepub zurück** (K4-Rest): XPointer → `#kobo.N.M:offset`
  (UTF-16-Einheiten), Fixtures Book P/Q/R; `pull --write` am Kobo per USB.
  - [x] **ich:** Übersetzung, Tests (PR #55; siehe K4).
  - [x] **du:** Book R vom Server per USB auf den Kobo schreiben, auswerfen,
    öffnen: geprüft 2026-10-31, öffnet an der Stelle (siehe K4).
- [ ] **C2 Basis-Stand und Vormerkliste** (nur Bibliothek und CLI)
  - [ ] **ich:** `.adds/NickelInkSync/state.json` (je Buch Basis-Stand,
    vorgemerkt); Drei-Wege-Regel in `sync.rs` mit `confirm`; `not-in-line.tsv`
    einmal übernehmen, dann entfällt sie; `sync --pending` sendet bzw. holt
    alle vorgemerkten Bücher; Maschinenzeilen für das Plugin. Tests mit dem
    Mock-Server, auch für den Wechsel vom heutigen Plugin.
- [ ] **C3 Zuordnungen auf dem Gerät** (ersetzt N6; braucht C1)
  - [ ] **ich:** Am Rechner `device prepare`: Index und Übersetzungstabellen
    aufs Gerät, Zuordnungen in beide Richtungen abgleichen (eine am Gerät
    angelegte Zuordnung findet ihr Original über die Prüfsumme); `--live`
    liest sie; Push ohne Index oder ohne Eintrag verweigert, mit eigener
    Maschinenzeile für die Meldung.
  - [ ] **du:** Calibre-kepub auf dem Kobo lesen, `push --live` per Plugin
    oder CLI, in KOReader mit der Original-EPUB öffnen.
- [ ] **C4 Plugin auf dem Kobo** (ersetzt N2; braucht C0, C2, C3)
  - [ ] **ich:** Plugin auf dem Kobo aktiv; Öffnen und Schließen über
    Nickels Schreibweg; offline vormerken, bei WLAN nachholen; kürzere
    Wartezeit; Meldung ohne Index. Paket `Kobo.tgz` für beide Geräte.
  - [ ] **du:** Am Clara BW: Buch mit WLAN öffnen (Stand vom Server),
    dann WLAN aus, lesen, schließen (vorgemerkt), WLAN an, ein anderes Buch
    öffnen (nachgeholt); in KOReader prüfen.
- [ ] **C5 Dialog bei Konflikten** (braucht C4)
  - [ ] **ich:** Qt-Dialog beim Öffnen mit beiden Ständen und drei Knöpfen.
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
- [ ] **(in D2, nach C3)** Knopf „Gerät vorbereiten“ in der GUI; beim
  Kopieren (E5) Index und Tabelle gleich mitschreiben.

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
