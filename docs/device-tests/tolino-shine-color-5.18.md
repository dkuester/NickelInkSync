# Gerätetest: Tolino Shine Color 5.18

Gerät: **Tolino Shine Color**, Firmware **5.18.279364**. Ein Termin deckt alle offenen Punkte für 0.1.0 ab
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
| 1 | **KOReader** (dein anderes Gerät), WLAN an: Book R öffnen und bis zu einer Stelle **S1** lesen, die mindestens 20 Seiten hinter dem Stand von Book R auf dem E-Reader liegt; Fortschritt senden. Danach KOReader **nicht mehr anfassen**, bis die Tabelle ausdrücklich KOReader nennt. | Server hat S1. Notiere S1 (z. B. R2-41). |  |
| 2 | **E-Reader**, WLAN **an**, verbunden: Book R öffnen. | Toast „Fortschritt übernommen“; das Buch öffnet auf der Seite von S1 (höchstens ein, zwei Absätze davor), ohne Neustart. |  |
| 3 | Book R schließen (zurück zur Bibliothek). | Toast „Fortschritt ist aktuell“; nichts wird gesendet. |  |
| 4 | WLAN am E-Reader **aus** (kein WLAN-Symbol). Book R öffnen, mindestens 20 Seiten weiterlesen (Stelle **S2**), Book R schließen, bis zu 15 s warten. | Beim Öffnen ohne WLAN der Toast „Laden fehlgeschlagen“ („Kein WLAN oder kein Server“): richtig. Kein WLAN-Dialog. Beim Schließen Toast „Fortschritt vorgemerkt“ (kann bis ~15 s nach dem Schließen kommen). Notiere S2. |  |
| 5 | Weiter ohne WLAN: Book P öffnen, **nicht blättern**, schließen. | Keine Meldung „vorgemerkt“; Book P wird nicht vorgemerkt. |  |
| 6 | WLAN **an**, warten bis verbunden. Ein **anderes** Buch öffnen als Book R (Book Q). | Etwa 5 s nach dem Öffnen Toast „Vorgemerktes gesendet“ mit **„1 Buch“** (nicht 2, Book P darf nicht dabei sein). |  |
| 7 | **KOReader:** Book R öffnen. | KOReader bietet Stelle S2 vom Server an (auf den Absatz genau); übernehmen, aber nichts weiterlesen. |  |
| 8 | **KOReader:** Book R auf Stelle **S3** blättern (ca. 5 Seiten hinter S2), Fortschritt senden. KOReader danach nicht mehr anfassen. | Server hat S3 (neuer als der Stand des E-Readers). |  |
| 9 | **E-Reader**, WLAN **aus**: Book R öffnen, bis Stelle **S4** lesen (mindestens 25 Seiten hinter S2, also **weiter** als S3), schließen, 15 s warten. | Toast „Fortschritt vorgemerkt“. (Hier ist der Konflikt vorbereitet: beide bewegt, Server neuer, aber weiter hinten.) |  |
| 10 | WLAN **an**, verbunden: Book R öffnen. | Toast „Konflikt, nicht geladen“ („Server und Gerät bewegt, bleibt liegen“); das Buch öffnet unverändert an S4. Kein Dialog (den gibt es erst in 0.1.1). Zusätzlich darf kurz „Vorgemerkt: Konflikt“ erscheinen. |  |
| 11 | Book R schließen. | Toast „Nicht gesendet“ (Konflikt). Nichts geht an den Server. |  |
| 12 | **KOReader:** Book R öffnen. | KOReader steht weiter auf S3 und bietet **nicht** S4 an: der Server wurde nicht überschrieben. |  |
| 13 | **Ohne `device prepare`:** E-Reader per USB anschließen, im Ordner `.adds/NickelInkSync/` die Datei `library.json` umbenennen in `library.json.aus`, auswerfen, E-Reader **neu starten** (aus- und einschalten), WLAN an, Book Q öffnen. | Toast „Gerät nicht vorbereitet“ (mit Hinweis „Am Rechner ‚Gerät vorbereiten‘“), **nicht** „Buch nicht zugeordnet“. Danach ein anderes Buch öffnen: die Meldung kommt nicht noch einmal (einmal pro Start). Beim Schließen Toast „Nicht gesendet / Gerät nicht vorbereitet“, nichts geht an den Server. (Ein Buch ohne Zuordnung bei vorbereitetem Gerät meldet dagegen „nicht zugeordnet“: ein anderer Fall.) |  |
| 14 | Aufräumen: auf dem Rechner `cargo run -q -- device prepare --device /Volumes/KOBOeReader` (stellt `library.json` wieder her; die Datei `library.json.aus` löschen). | Ausgabe „Prepared … originals in the index …“. `just device-check /Volumes/KOBOeReader` zeigt wieder ✓ bei „Prepared“. |  |
| 15 | Am Ende per USB das Log ansehen: `grep -n -e "after opening" -e conflict -e inactive /Volumes/KOBOeReader/.adds/NickelInkSync/NickelInkSync.log` | **Wartezeit:** zu Schritt 2 steht eine Zeile „after opening (N ms): Nickel is at …, pulled …“, N zwischen 3000 und 15000; trage N ein. Kein „Nicht übernommen“ ohne Anlass. |  |

Hinweis: Der Tolino schreibt über den alten Weg (die CLI schreibt, `--write`); nach dem Schreiben reicht Auswerfen bzw. das Plugin, kein Neustart. Kobalt wird hier nicht geprüft (Kobalt-Schutz am Libra 2 und am Clara BW).

## Nach dem Test

- `just device-check /Volumes/KOBOeReader` noch einmal laufen lassen und die Ausgabe zu den Ergebnissen legen.
- Pfad `.adds/NickelInkSync/ignore-kobalt` darf am Ende nicht existieren (außer du willst Kobalt daneben).
