# Gerätetest: Kobo Clara BW 6.0 (C6)

Gerät: Kobo Clara BW, Firmware **6.0.279225** (Qt 6). Ein Termin deckt die Oberfläche (C6) ab. Trage in der Spalte „Ergebnis“ ein, was du siehst, und schick mir die Tabelle
(bei Abweichungen die Zeilen aus `NickelInkSync.log`, **nie** `config.ini`). Der Tolino kommt in C7.

## Voraussetzungen

1. **Rechner, Ordner und Branch:**

   ```bash
   cd ~/GitHub/inksync
   git fetch && git switch claude/c6-surface && git pull
   ```

   Der Ordner muss sauber sein (`git status`). Nach dem Merge: `git switch main && git pull`.

2. **Paket frisch bauen** (Docker muss laufen):

   ```bash
   just plugin && just package
   ```

3. **E-Reader** per USB anschließen (hier `/Volumes/KOBOeReader`), `just device-check /Volumes/KOBOeReader` ansehen.

4. **Kopieren** (`config.ini`, `library.json`, `state.json` bleiben):

   ```bash
   cp -R dist/NickelInkSync/.adds/NickelInkSync /Volumes/KOBOeReader/.adds/
   cp dist/NickelInkSync/Kobo.tgz /Volumes/KOBOeReader/.kobo/
   ```

   **NickelMenu:** der Qt-6-Fork von nicoverbruggen (auf 6.0 gibt es NickelMenu nur so). Dann `cp dist/NickelInkSync/.adds/nm/nickelinksync /Volumes/KOBOeReader/.adds/nm/`.

5. **Konfiguration:** `config.ini` mit Konto; `confirm = backwards` oder weglassen, `notify = all` oder weglassen.

6. **Gerät vorbereiten** (Book R als kepub auf dem Gerät, **nicht** zugeordnet; ein zweites nicht zugeordnetes Buch ist gut):

   ```bash
   cargo run -q -- device prepare --device /Volumes/KOBOeReader
   ```

   Auswerfen; das Gerät installiert das Plugin und startet neu. Warten, WLAN an.

## Schritte

| Nr. | Schritt | Erwartetes Ergebnis | Ergebnis (du füllst aus) |
|---|---|---|---|
| 1 | NickelMenu öffnen, **NickelInkSync** wählen. | Nach ein, zwei Sekunden der Dialog „NickelInkSync“: „Vorgemerkt: nichts“, „Ohne Zuordnung: n“ mit den Büchern, Knöpfe „Jetzt synchronisieren“, „Zuordnen …“, „Schließen“. | |
| 2 | „Schließen“. | Der Dialog verschwindet, nichts passiert. | |
| 3 | WLAN **aus**. Ein zugeordnetes oder originales Buch öffnen, weiterlesen, schließen („Fortschritt vorgemerkt“). Dann WLAN **an**, NickelMenu **NickelInkSync jetzt synchronisieren**. | Toast „Sende Vorgemerktes …“, dann „Vorgemerktes gesendet / 1 Buch“. KOReader zeigt die Stelle. (Vorher im Dialog sichtbar: „Vorgemerkt: 1“ mit dem Titel.) | |
| 4 | Noch einmal „NickelInkSync jetzt synchronisieren“. | Toast „Nichts vorgemerkt / Alles ist abgeglichen“. | |
| 5 | NickelMenu **NickelInkSync**, „Zuordnen …“. | Dialog „Buch zuordnen“: „Auf dem Gerät: Titel – Autor“, „Passt zu: …“ mit Prozent und „Vorschlag 1 von n“, Knöpfe „Zuordnen“, „Nächster Vorschlag“, „Abbrechen“. Der erste Vorschlag ist der richtige Originaltitel. | |
| 6 | „Nächster Vorschlag“. | Der nächste Vorschlag erscheint; beim letzten heißt der Knopf „Überspringen“. | |
| 7 | Zurück zum richtigen Vorschlag, „Zuordnen“. | Toast „Zugeordnet / Am Rechner ‚Gerät vorbereiten‘“. Danach der nächste nicht zugeordnete Buchtitel oder „Kein Vorschlag“. | |
| 8 | Das kepub öffnen, kurz lesen, schließen. | Beim Öffnen „Nicht übernommen / Zuordnung nicht vorbereitet“, beim Schließen „Nicht gesendet / Zuordnung nicht vorbereitet“: Es ist zugeordnet, aber die Tabelle fehlt noch (erwartet; gesendet wird erst nach Schritt 9). | |
| 9 | Auswerfen/USB: am Rechner `cargo run -q -- device prepare --device /Volumes/KOBOeReader` (Ausgabe „Imported … (mapped on the device)“, „Tables 1 written“), auswerfen. Das kepub öffnen, ein paar Seiten lesen, schließen. | Fortschritt wird gesendet („Fortschritt gesendet“); KOReader zeigt die Stelle im Original-EPUB. | |
| 10 | Bibliothek → bei einem Buch **⋯** (Buchmenü). | Der letzte Eintrag heißt **NickelInkSync**. Bei einem nicht zugeordneten Buch führt er in „Buch zuordnen“; bei einem zugeordneten Toast „Wird synchronisiert“ bzw. „Zugeordnet, wartet“. | |
| 11 | Ohne `library.json` (umbenennen in `library.json.aus`, auswerfen, neu starten): NickelMenu **NickelInkSync**. | Toast „Gerät nicht vorbereitet“ („Am Rechner ‚Gerät vorbereiten‘“), kein Dialog. Danach `library.json` zurück (`device prepare`). | |
| 12 | **Kobalt-Schutz:** Kobalt installiert (ohne `ignore-kobalt`), neu starten, NickelMenu **NickelInkSync**. | Nur der Toast „NickelInkSync ist aus“ (einmal), kein Dialog; im Buchmenü kein Eintrag **NickelInkSync**. Kobalt wieder entfernen. | |

## Nach dem Test

- `grep -n -e dialog -e trigger -e "device " /Volumes/KOBOeReader/.adds/NickelInkSync/NickelInkSync.log` zu den Ergebnissen legen.
- Falls NickelMenu den Befehl nicht ausführt (Schritt 1 ohne Reaktion): im Log fehlt die Zeile „trigger: menu“; dann hilft der
  Eintrag im Buchmenü (Schritt 10) und die Syntax der NickelMenu-Datei ist zu prüfen (Zeilen `cmd_spawn`).
