# Gerätetest: Tolino Shine Color 5.18

Gerät: **Tolino Shine Color**, Firmware **5.18.279364**. **Durchgeführt am 2026-11-08, Ergebnisse eingetragen.** Ein Termin deckt alle offenen Punkte für 0.1.0 ab
(Retest mit dem Paket aus `main` nach den gemeinsamen Plugin-Änderungen seit C4, ohne Dialog; die längere Wartezeit „after opening (… ms)“). Trage in der Spalte „Ergebnis“ ein, was du siehst, und schick mir die Tabelle
(und bei Abweichungen die Ausgabe von `just device-check` und die Zeilen aus `NickelInkSync.log`,
**nie** `config.ini`).

## Voraussetzungen

1. **Rechner, Ordner und Branch** (Terminal):

   ```bash
   cd ~/GitHub/inksync
   git switch main && git pull
   ```

   Der Ordner muss sauber sein (`git status`). Das Gerätepaket kommt immer aus dem aktuellen `main`.

2. **Paket frisch bauen** (Docker muss laufen; ein altes Paket in `dist/` hat schon Testrunden verdorben):

   ```bash
   just plugin && just package
   ```

3. **E-Reader** per USB anschließen (hier eingehängt als `/Volumes/KOBOeReader`; stimmt der Pfad nicht, siehe `ls /Volumes`), dann Zustand ansehen:

   ```bash
   just device-check /Volumes/KOBOeReader
   ```

   Das ändert nichts. Mit altem Paket stehen jetzt ✗-Zeilen bei „Plugin“ oder „Program“; das ist hier in Ordnung, Schritt 4 holt es nach.

4. **Programm und Plugin aufs Gerät kopieren** (überschreibt nichts Eigenes: `config.ini`, `library.json` und `state.json` bleiben liegen):

   ```bash
   cp -R dist/NickelInkSync/.adds/NickelInkSync /Volumes/KOBOeReader/.adds/
   cp dist/NickelInkSync/Kobo.tgz /Volumes/KOBOeReader/.kobo/
   ```

5. **Konfiguration** in `/Volumes/KOBOeReader/.adds/NickelInkSync/config.ini` (mit dem Editor öffnen, das Passwort **nicht** herzeigen): `server`, `user` und `password` oder `key` sind gesetzt, und

   - `confirm = backwards` (oder die Zeile ganz weglassen). **Nicht** `confirm = never`: dann gibt es nie einen Konflikt, und der Konflikttest ist verpufft.
   - `notify = all` (oder die Zeile weglassen).

6. **Gerät vorbereiten** (Bibliothek aus der Konfiguration des Rechners; Book R, P und Q müssen darin sein und Book R als kepub dem EPUB zugeordnet):

   ```bash
   cargo run -q -- device prepare --device /Volumes/KOBOeReader
   ```

7. **Noch einmal prüfen**, jetzt vor dem Auswerfen:

   ```bash
   just device-check /Volumes/KOBOeReader
   ```

   Erwartet: alles ✓, außer ggf. „!“ bei „Pending“. Bei einem ✗ **nicht testen**, sondern beheben (die Zeile sagt wie).

8. E-Reader **auswerfen**. Das Gerät installiert das Plugin und startet neu. Warten, bis die Bibliothek wieder da ist; WLAN an.

**Während des Tests:** KOReader und der E-Reader dürfen nicht gleichzeitig dasselbe Buch senden. Genau die Reihenfolge unten einhalten, sie löst die Fälle aus.

## Schritte

| Nr. | Schritt | Erwartetes Ergebnis | Ergebnis (du füllst aus) |
|---|---|---|---|
| 1 | **KOReader** (dein anderes Gerät), WLAN an: Book R öffnen und bis zu einer Stelle **S1** lesen, die mindestens 20 Seiten hinter dem Stand von Book R auf dem E-Reader liegt; Fortschritt senden. Danach KOReader **nicht mehr anfassen**, bis die Tabelle ausdrücklich KOReader nennt. | Server hat S1. Notiere S1 (z. B. R2-41). | ✓ KOReader bei R2-188 (S1) gesendet. |
| 2 | **E-Reader**, WLAN **an**, verbunden: Book R öffnen. | Toast „Fortschritt übernommen“; das Buch öffnet auf der Seite von S1 (höchstens ein, zwei Absätze davor), ohne Neustart. | ✓ „Fortschritt übernommen“, Position R2-186. Log: `after opening (3000 ms) … on the page with the pulled point`. |
| 3 | Book R schließen (zurück zur Bibliothek). | Toast „Fortschritt ist aktuell“; nichts wird gesendet. | ✓ „Fortschritt ist aktuell“. |
| 4 | WLAN am E-Reader **aus** (kein WLAN-Symbol). Book R öffnen, mindestens 20 Seiten weiterlesen (Stelle **S2**), Book R schließen, bis zu 15 s warten. | Beim Öffnen ohne WLAN der Toast „Laden fehlgeschlagen“ („Kein WLAN oder kein Server“): richtig. Kein WLAN-Dialog. Beim Schließen Toast „Fortschritt vorgemerkt“ (kann bis ~15 s nach dem Schließen kommen). Notiere S2. | ✓ Öffnen ohne WLAN „Laden fehlgeschlagen“, bis R3-8 gelesen (S2), geschlossen: „Fortschritt vorgemerkt“. |
| 5 | Weiter ohne WLAN: Book P öffnen, **nicht blättern**, schließen. | Keine Meldung „vorgemerkt“; Book P wird nicht vorgemerkt. | ✓ Good Boy (statt Book P) offline: „Laden fehlgeschlagen“, nichts vorgemerkt. |
| 6 | WLAN **an**, warten bis verbunden. Ein **anderes** Buch öffnen als Book R (Book Q). | Etwa 5 s nach dem Öffnen Toast „Vorgemerktes gesendet“ mit **„1 Buch“** (nicht 2, Book P darf nicht dabei sein). | ✓ Mayfair Witches (statt Book Q): „Fortschritt ist aktuell“, dann „Vorgemerktes gesendet, 1 Buch“. Beim Verlassen kam „Nicht gesendet / Konflikt“ (Log 15:41:53, device 8 %, server 13 % von Readest): richtig, das Buch hatte keinen gemeinsamen Stand, das Gerät war durch das Öffnen neuer und lag weiter hinten. |
| 7 | **KOReader:** Book R öffnen. | KOReader bietet Stelle S2 vom Server an (auf den Absatz genau); übernehmen, aber nichts weiterlesen. | ✓ KOReader lädt bei R3-5 (Absatzanfang vor R3-8). |
| 8 | **Zuerst das Gerät:** **E-Reader**, WLAN **aus**: Book R öffnen, bis Stelle **S4** lesen (mindestens 25 Seiten hinter dem Stand, der gerade mit dem Server abgeglichen ist), schließen, 15 s warten. KOReader darf davor **nicht** gesendet haben. | Toast „Fortschritt vorgemerkt“. (Das Lesen am Gerät ist jetzt älter als alles, was KOReader gleich sendet.) | ✓ (richtige Reihenfolge) WLAN aus, Book R bis R3-26 (S4), geschlossen: „Fortschritt vorgemerkt“. |
| 9 | **Danach KOReader:** Book R öffnen (fragt KOReader nach dem Server-Stand: ablehnen, am eigenen Stand bleiben), auf Stelle **S3** blättern, die **vor S4** liegt (ca. 10 Seiten davor) und vom abgeglichenen Stand abweicht, Fortschritt senden. Danach nicht mehr anfassen. | Server hat S3, **neuer** als das Lesen am E-Reader, aber weiter hinten als S4. | ✓ KOReader bis R3-14 (S3) geblättert und gesendet. |
| 10 | WLAN **an**, verbunden: Book R öffnen. | Toast „Konflikt, nicht geladen“ („Server und Gerät bewegt, bleibt liegen“); das Buch öffnet unverändert an S4. Kein Dialog (den gibt es erst in 0.1.1). Zusätzlich darf kurz „Vorgemerkt: Konflikt“ erscheinen. | ✓ „Konflikt, nicht geladen“ (Log 15:45:07: both-moved), Position bleibt R3-26. |
| 11 | Book R schließen. | Toast „Nicht gesendet“ (Konflikt). Nichts geht an den Server. | ✓ „Nicht gesendet“ („Konflikt, bleibt liegen“). |
| 12 | **KOReader:** Book R öffnen. | KOReader steht weiter auf S3 und bietet **nicht** S4 an: der Server wurde nicht überschrieben. | ✓ KOReader bleibt bei R3-14. |
| 13 | **Ohne `device prepare`:** E-Reader per USB anschließen, im Ordner `.adds/NickelInkSync/` die Datei `library.json` umbenennen in `library.json.aus`, auswerfen, E-Reader **neu starten** (aus- und einschalten), WLAN an, Book Q öffnen. | Toast „Gerät nicht vorbereitet“ (mit Hinweis „Am Rechner ‚Gerät vorbereiten‘“), **nicht** „Buch nicht zugeordnet“. Danach ein anderes Buch öffnen: die Meldung kommt nicht noch einmal (einmal pro Start). Beim Schließen Toast „Nicht gesendet / Gerät nicht vorbereitet“, nichts geht an den Server. (Ein Buch ohne Zuordnung bei vorbereitetem Gerät meldet dagegen „nicht zugeordnet“: ein anderer Fall.) | ✓ mit einer Abweichung zu Kobo: Book R (ein EPUB, Original) meldet beim Öffnen „Fortschritt ist aktuell“ und erst beim Schließen „Gerät nicht vorbereitet / Am Rechner ‚Gerät vorbereiten‘“ (Log 15:48:19). Das Holen bleibt ohne Index erlaubt, das Plugin merkt es erst beim ersten Senden. Good Boy danach: keine zweite Meldung (einmal pro Start). Bei einem kepub kommt die Meldung schon beim Öffnen (Clara, Libra). |
| 14 | Aufräumen: auf dem Rechner `cargo run -q -- device prepare --device /Volumes/KOBOeReader` (stellt `library.json` wieder her; die Datei `library.json.aus` löschen). | Ausgabe „Prepared … originals in the index …“. `cargo run -q -- device check --device /Volumes/KOBOeReader` zeigt wieder ✓ bei „Prepared“. | ✓ `device prepare` mit der Calibre-Bibliothek: 1131 Originale. `library.json.aus` ist noch zu löschen. |
| 15 | Am Ende per USB das Log ansehen: `grep -n -e "after opening" -e conflict -e inactive /Volumes/KOBOeReader/.adds/NickelInkSync/NickelInkSync.log` | **Wartezeit:** zu Schritt 2 steht eine Zeile „after opening (N ms): Nickel is at …, pulled …“, N zwischen 3000 und 15000; trage N ein. Kein „Nicht übernommen“ ohne Anlass. | ✓ Wartezeit: `after opening (3000 ms)`, Nickel kannte seine Stelle sofort; die längere Wartezeit (bis 15 s) wurde nicht gebraucht. Kein Fehler im Log. |

**Befund zur Vorbereitung:** Book R liegt nicht in der Calibre-Bibliothek, sondern in `/Users/daniel/KOreader/`. Das Plugin sendet nur Originale aus dem Index (`library.json`) oder zugeordnete Bücher; mit der Calibre-Bibliothek meldete Book R daher „Nicht zugeordnet“. Für den Test wurde der Tolino mit `cargo run -q -- device prepare --device /Volumes/eReader --library /Users/daniel/KOreader` vorbereitet (12 Originale), für Schritt 14 danach wieder mit Calibre. Auf dem Tolino lagen nur Book R (EPUB), Good Boy und Mayfair Witches; sie ersetzen Book P und Q.

Hinweis: Der Tolino schreibt über den alten Weg (die CLI schreibt, `--write`); nach dem Schreiben reicht Auswerfen bzw. das Plugin, kein Neustart. Kobalt wird hier nicht geprüft (Kobalt-Schutz am Libra 2 und am Clara BW).

Warum die Reihenfolge beim Konflikt so ist: Ein Konflikt entsteht nur, wenn der Server-Stand **neuer** ist als das letzte Lesen am Gerät *und* weiter hinten liegt. Sendet KOReader schon vor dem Offline-Lesen, ist das Gerät neuer und gewinnt: „Fortschritt gesendet“ (so am Libra 2 am 2026-11-07 gesehen; das ist die Regel, kein Fehler).

## Nach dem Test

- `just device-check /Volumes/KOBOeReader` noch einmal laufen lassen und die Ausgabe zu den Ergebnissen legen.
- Pfad `.adds/NickelInkSync/ignore-kobalt` darf am Ende nicht existieren (außer du willst Kobalt daneben).
