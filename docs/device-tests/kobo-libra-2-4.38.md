# Gerätetest: Kobo Libra 2 4.38

Gerät: **Kobo Libra 2**, Firmware **4.38.23852**. Ein Termin deckt alle offenen Punkte für 0.1.0 ab
(Kobalt-Schutz und die Meldung „Gerät nicht vorbereitet“, die offenen C10-Punkte; dazu der volle Durchlauf ohne Dialog). Trage in der Spalte „Ergebnis“ ein, was du siehst, und schick mir die Tabelle
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
   just plugin-qt5 && just package
   ```

3. **E-Reader** per USB anschließen (hier eingehängt als `/Volumes/KOBOeReader`; stimmt der Pfad nicht, siehe `ls /Volumes`), dann Zustand ansehen:

   ```bash
   just device-check /Volumes/KOBOeReader
   ```

   Das ändert nichts. Mit altem Paket stehen jetzt ✗-Zeilen bei „Plugin“ oder „Program“; das ist hier in Ordnung, Schritt 4 holt es nach.

4. **Programm und Plugin aufs Gerät kopieren** (überschreibt nichts Eigenes: `config.ini`, `library.json` und `state.json` bleiben liegen):

   ```bash
   cp -R dist/NickelInkSync/.adds/NickelInkSync /Volumes/KOBOeReader/.adds/
   cp plugin/KoboRoot.tgz /Volumes/KOBOeReader/.kobo/
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

   (Für Kobo 4.x kommt **nur** `KoboRoot.tgz` nach `.kobo/`, nicht `Kobo.tgz`.)

8. E-Reader **auswerfen**. Das Gerät installiert das Plugin und startet neu. Warten, bis die Bibliothek wieder da ist; WLAN an.

**Während des Tests:** KOReader und der E-Reader dürfen nicht gleichzeitig dasselbe Buch senden. Genau die Reihenfolge unten einhalten, sie löst die Fälle aus.

## Schritte

| Nr. | Schritt | Erwartetes Ergebnis | Ergebnis (du füllst aus) |
|---|---|---|---|
| 1 | **KOReader** (dein anderes Gerät), WLAN an: Book R öffnen und bis zu einer Stelle **S1** lesen, die mindestens 20 Seiten hinter dem Stand von Book R auf dem E-Reader liegt; Fortschritt senden. Danach KOReader **nicht mehr anfassen**, bis die Tabelle ausdrücklich KOReader nennt. | Server hat S1. Notiere S1 (z. B. R2-41). | ✓ KOReader sendet R1-10. |
| 2 | **E-Reader**, WLAN **an**, verbunden: Book R öffnen. | Toast „Fortschritt übernommen“; das Buch öffnet auf der Seite von S1 (höchstens ein, zwei Absätze davor), ohne Neustart. | ✓ „Fortschritt übernommen“, Reader bei R1-7 (4.x öffnet am Span-Anfang). Der erste Versuch kurz nach dem Neustart gab „Server antwortet nicht“ (Timeout nach 5 s, Server vom Mac in 0,1 s erreichbar): zu früh nach dem Start; dann ✓. |
| 3 | Book R schließen (zurück zur Bibliothek). | Toast „Fortschritt ist aktuell“; nichts wird gesendet. | ✓ „Fortschritt ist aktuell“. |
| 4 | WLAN am E-Reader **aus** (kein WLAN-Symbol). Book R öffnen, mindestens 20 Seiten weiterlesen (Stelle **S2**), Book R schließen, bis zu 15 s warten. | Beim Öffnen ohne WLAN der Toast „Laden fehlgeschlagen“ („Kein WLAN oder kein Server“): richtig. Kein WLAN-Dialog. Beim Schließen Toast „Fortschritt vorgemerkt“ (kann bis ~15 s nach dem Schließen kommen). Notiere S2. | ✓ Öffnen ohne WLAN: „Laden fehlgeschlagen“. Bis R1-97 gelesen (S2), geschlossen: „Fortschritt vorgemerkt“. |
| 5 | Weiter ohne WLAN: Book P öffnen, **nicht blättern**, schließen. | Keine Meldung „vorgemerkt“; Book P wird nicht vorgemerkt. | ✓ Book P offline: „Nicht übernommen“, sonst keine Meldung (nichts vorgemerkt). |
| 6 | WLAN **an**, warten bis verbunden. Ein **anderes** Buch öffnen als Book R (Book Q). | Etwa 5 s nach dem Öffnen Toast „Vorgemerktes gesendet“ mit **„1 Buch“** (nicht 2, Book P darf nicht dabei sein). | ✓ Book Q: „Nicht übernommen“, dann „Vorgemerktes gesendet, 1 Buch“. |
| 7 | **KOReader:** Book R öffnen. | KOReader bietet Stelle S2 vom Server an (auf den Absatz genau); übernehmen, aber nichts weiterlesen. | ✓ KOReader lädt bei R1-94 (Absatzanfang vor R1-97). |
| 8 | **Zuerst das Gerät:** **E-Reader**, WLAN **aus**: Book R öffnen, bis Stelle **S4** lesen (mindestens 25 Seiten hinter dem Stand, der gerade mit dem Server abgeglichen ist), schließen, 15 s warten. KOReader darf davor **nicht** gesendet haben. | Toast „Fortschritt vorgemerkt“. (Das Lesen am Gerät ist jetzt älter als alles, was KOReader gleich sendet.) | **Wiederholen.** Erster Versuch in der alten, falschen Reihenfolge (KOReader sendete R1-117, dann am Gerät offline bis R1-147): beim Öffnen „Fortschritt ist aktuell“ und „Fortschritt gesendet, 1 Buch“, **kein Konflikt**. Das ist die Regel (das Gerät war neuer), kein Fehler; die Tabelle war falsch. Jetzt gilt: zuerst offline am Gerät lesen, **dann** KOReader senden. |
| 9 | **Danach KOReader:** Book R öffnen (fragt KOReader nach dem Server-Stand: ablehnen, am eigenen Stand bleiben), auf Stelle **S3** blättern, die **vor S4** liegt (ca. 10 Seiten davor) und vom abgeglichenen Stand abweicht, Fortschritt senden. Danach nicht mehr anfassen. | Server hat S3, **neuer** als das Lesen am E-Reader, aber weiter hinten als S4. | Offen (Wiederholung). |
| 10 | WLAN **an**, verbunden: Book R öffnen. | Toast „Konflikt, nicht geladen“ („Server und Gerät bewegt, bleibt liegen“); das Buch öffnet unverändert an S4. Kein Dialog (den gibt es erst in 0.1.1). Zusätzlich darf kurz „Vorgemerkt: Konflikt“ erscheinen. | Offen (Wiederholung). |
| 11 | Book R schließen. | Toast „Nicht gesendet“ (Konflikt). Nichts geht an den Server. | Offen (Wiederholung). |
| 12 | **KOReader:** Book R öffnen. | KOReader steht weiter auf S3 und bietet **nicht** S4 an: der Server wurde nicht überschrieben. | Offen (Wiederholung). |
| 13 | **Ohne `device prepare`:** E-Reader per USB anschließen, im Ordner `.adds/NickelInkSync/` die Datei `library.json` umbenennen in `library.json.aus`, auswerfen, E-Reader **neu starten** (aus- und einschalten), WLAN an, Book Q öffnen. | Toast „Gerät nicht vorbereitet“ (mit Hinweis „Am Rechner ‚Gerät vorbereiten‘“), **nicht** „Buch nicht zugeordnet“. Danach ein anderes Buch öffnen: die Meldung kommt nicht noch einmal (einmal pro Start). Beim Schließen Toast „Nicht gesendet / Gerät nicht vorbereitet“, nichts geht an den Server. (Ein Buch ohne Zuordnung bei vorbereitetem Gerät meldet dagegen „nicht zugeordnet“: ein anderer Fall.) |  |
| 14 | Aufräumen: auf dem Rechner `cargo run -q -- device prepare --device /Volumes/KOBOeReader` (stellt `library.json` wieder her; die Datei `library.json.aus` löschen). | Ausgabe „Prepared … originals in the index …“. `just device-check /Volumes/KOBOeReader` zeigt wieder ✓ bei „Prepared“. |  |
| 15 | **Kobalt-Schutz:** Kobalt (Version für diese Firmware, Anleitung bei nicoverbruggen/kobalt) per USB installieren, wie dort beschrieben. Die Datei `.adds/NickelInkSync/ignore-kobalt` darf **nicht** existieren. Dann `just device-check /Volumes/KOBOeReader`. | Zeile „! Kobalt installed: the plugin stays inactive and sends nothing“. |  |
| 16 | Auswerfen, E-Reader neu starten (Kobalt ist dann aktiv), WLAN an, Book R öffnen, ein paar Seiten blättern, schließen. | Genau **einmal** der Toast „NickelInkSync ist aus“ (Kobalt ist installiert); sonst keine NickelInkSync-Meldung, kein „übernommen“, kein „gesendet“, kein „vorgemerkt“. Der Server bleibt unverändert. |  |
| 17 | Log: `grep -n inactive /Volumes/KOBOeReader/.adds/NickelInkSync/NickelInkSync.log` | Zeile mit „inactive: Kobalt is installed“. |  |
| 18 | Aufräumen: Kobalt wieder entfernen (`.adds/kobalt/uninstall` löschen, auswerfen, neu starten). Danach `just device-check /Volumes/KOBOeReader`. | Zeile „✓ Kobalt …“ (nicht installiert); Book R öffnen zeigt wieder die Meldungen von NickelInkSync. |  |
| 19 | Am Ende per USB das Log ansehen: `grep -n -e "after opening" -e "opening hook" -e inactive -e conflict /Volumes/KOBOeReader/.adds/NickelInkSync/NickelInkSync.log` | Je Öffnen eine Zeile „after opening (… ms): Nickel is at …“. Keine Zeile „Fehler“/„error“ ohne erkennbaren Anlass. |  |

Hinweis: Die Schritte mit Kobalt kommen zuletzt, weil Kobalt das Plugin lahmlegt.

Warum die Reihenfolge beim Konflikt so ist: Ein Konflikt entsteht nur, wenn der Server-Stand **neuer** ist als das letzte Lesen am Gerät *und* weiter hinten liegt. Sendet KOReader schon vor dem Offline-Lesen, ist das Gerät neuer und gewinnt: „Fortschritt gesendet“ (so am Libra 2 am 2026-11-07 gesehen; das ist die Regel, kein Fehler).

## Nach dem Test

- `just device-check /Volumes/KOBOeReader` noch einmal laufen lassen und die Ausgabe zu den Ergebnissen legen.
- Pfad `.adds/NickelInkSync/ignore-kobalt` darf am Ende nicht existieren (außer du willst Kobalt daneben).
