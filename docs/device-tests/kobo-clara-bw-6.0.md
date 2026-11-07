# Gerätetest: Kobo Clara BW 6.0

Gerät: **Kobo Clara BW**, Firmware **6.0.279225**. **Durchgeführt am 2026-11-07, Ergebnisse unten eingetragen.** Ein Termin deckt alle offenen Punkte für 0.1.0 ab
(Kobalt-Schutz und „Gerät nicht vorbereitet“ in Kurzform, dazu ein kurzer Durchlauf). Trage in der Spalte „Ergebnis“ ein, was du siehst, und schick mir die Tabelle
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
| 1 | **KOReader** (dein anderes Gerät), WLAN an: Book R öffnen und bis zu einer Stelle **S1** lesen, die mindestens 20 Seiten hinter dem Stand von Book R auf dem E-Reader liegt; Fortschritt senden. Danach KOReader **nicht mehr anfassen**, bis die Tabelle ausdrücklich KOReader nennt. | Server hat S1. Notiere S1 (z. B. R2-41). | S1 = R1-5 (Ausgangslage: Anfang Kapitel 1; erst R1-48 gesendet und geladen, dann auf R1-5 zurück und gesendet). ✓ |
| 2 | **E-Reader**, WLAN **an**, verbunden: Book R öffnen. | Toast „Fortschritt übernommen“; das Buch öffnet auf der Seite von S1 (höchstens ein, zwei Absätze davor), ohne Neustart. | ✓ „Fortschritt übernommen“, Reader bei R1-3. Log: after opening (3000 ms) … on the page with the pulled point. |
| 3 | Book R schließen (zurück zur Bibliothek). | Toast „Fortschritt ist aktuell“; nichts wird gesendet. | ✓ „Fortschritt ist aktuell“. |
| 4 | WLAN am E-Reader **aus** (kein WLAN-Symbol). Book R öffnen, mindestens 20 Seiten weiterlesen (Stelle **S2**), Book R schließen, bis zu 15 s warten. | Beim Öffnen ohne WLAN der Toast „Laden fehlgeschlagen“ („Kein WLAN oder kein Server“): richtig. Kein WLAN-Dialog. Beim Schließen Toast „Fortschritt vorgemerkt“ (kann bis ~15 s nach dem Schließen kommen). Notiere S2. | ✓ Öffnen ohne WLAN: „Laden fehlgeschlagen“. Bis R1-53 gelesen (S2), geschlossen: „Fortschritt vorgemerkt“. |
| 5 | WLAN **an**, warten bis verbunden. Ein **anderes** Buch öffnen als Book R (Book Q). | Etwa 5 s nach dem Öffnen Toast „Vorgemerktes gesendet“ mit **„1 Buch“** (nicht 2, Book P darf nicht dabei sein). | ✓ „Vorgemerktes gesendet“. Geöffnet wurde „Good Boy“ (nicht zugeordnet) statt Book Q: dazu „nicht zugeordnet“ (richtig bei vorbereitetem Gerät). Gesendet wurde 1 Buch. |
| 6 | **KOReader:** Book R öffnen. | KOReader bietet Stelle S2 vom Server an (auf den Absatz genau); übernehmen, aber nichts weiterlesen. | ✓ KOReader steht auf R1-50 (Absatzanfang vor R1-53). |
| 7 | **Ohne `device prepare`:** E-Reader per USB anschließen, im Ordner `.adds/NickelInkSync/` die Datei `library.json` umbenennen in `library.json.aus`, auswerfen, E-Reader **neu starten** (aus- und einschalten), WLAN an, Book Q öffnen. | Toast „Gerät nicht vorbereitet“ (mit Hinweis „Am Rechner ‚Gerät vorbereiten‘“), **nicht** „Buch nicht zugeordnet“. Danach ein anderes Buch öffnen: die Meldung kommt nicht noch einmal (einmal pro Start). Beim Schließen Toast „Nicht gesendet / Gerät nicht vorbereitet“, nichts geht an den Server. (Ein Buch ohne Zuordnung bei vorbereitetem Gerät meldet dagegen „nicht zugeordnet“: ein anderer Fall.) | ✓ (Log 13:52) Öffnen: „Gerät nicht vorbereitet / Am Rechner ‚Gerät vorbereiten‘“; Schließen: „Nicht gesendet / Gerät nicht vorbereitet“. Der erste Versuch mit „Good Boy“ prüfte nur „nicht zugeordnet“ und gilt nicht. |
| 8 | Aufräumen: auf dem Rechner `cargo run -q -- device prepare --device /Volumes/KOBOeReader` (stellt `library.json` wieder her; die Datei `library.json.aus` löschen). | Ausgabe „Prepared … originals in the index …“. `just device-check /Volumes/KOBOeReader` zeigt wieder ✓ bei „Prepared“. | ✓ `library.json` wieder da (13:52). `library.json.aus` liegt noch auf dem Gerät und ist zu löschen. |
| 9 | **Kobalt-Schutz:** Kobalt (Version für diese Firmware, Anleitung bei nicoverbruggen/kobalt) per USB installieren, wie dort beschrieben. Die Datei `.adds/NickelInkSync/ignore-kobalt` darf **nicht** existieren. Dann `just device-check /Volumes/KOBOeReader`. | Zeile „! Kobalt installed: the plugin stays inactive and sends nothing“. | Kobalt installiert; Toast „NickelInkSync ist aus“. `just device-check` lief nicht (Rezeptfehler, PR #86), nicht verifiziert. |
| 10 | Auswerfen, E-Reader neu starten (Kobalt ist dann aktiv), WLAN an, Book R öffnen, ein paar Seiten blättern, schließen. | Genau **einmal** der Toast „NickelInkSync ist aus“ (Kobalt ist installiert); sonst keine NickelInkSync-Meldung, kein „übernommen“, kein „gesendet“, kein „vorgemerkt“. Der Server bleibt unverändert. | ✓ Book R („Read with Nickel“) geöffnet, kein weiterer Toast (der kam schon bei der Installation). |
| 11 | Log: `grep -n inactive /Volumes/KOBOeReader/.adds/NickelInkSync/NickelInkSync.log` | Zeile mit „inactive: Kobalt is installed“. | ✓ Log 13:38:41 und 13:42:28: `inactive: Kobalt is installed …`. |
| 12 | Aufräumen: Kobalt wieder entfernen (`.adds/kobalt/uninstall` löschen, auswerfen, neu starten). Danach `just device-check /Volumes/KOBOeReader`. | Zeile „✓ Kobalt …“ (nicht installiert); Book R öffnen zeigt wieder die Meldungen von NickelInkSync. | Kobalt entfernt; Toast „NickelInkSync ist an“ (nicht aus dem Plugin, siehe Hinweis). `just device-check` lief bei dir nicht; Prüfung danach von Claude (nur lesend): Kobalt nicht installiert, alles ✓. |
| 13 | Am Ende per USB das Log ansehen: `grep -n -e "after opening" -e "opening hook" -e inactive -e conflict /Volumes/KOBOeReader/.adds/NickelInkSync/NickelInkSync.log` | Je Öffnen eine Zeile „after opening (… ms): Nickel is at …“. Keine Zeile „Fehler“/„error“ ohne erkennbaren Anlass. | ✓ Je Öffnen „after opening (3000 ms) … on the page with the pulled point“; Hinweise „opening hook: another mod (libstorygraph.so) … put back in front“ (NickelStorygraph verkettet ohne Schleife). Keine Fehlerzeilen. |

Hinweis: Kurzform. Den Konflikt und das Nachholen mehrerer Bücher hast du am Clara BW schon im Lauf von C4 geprüft; der Libra 2 und der Tolino haben die volle Tabelle.

## Nach dem Test

- `just device-check /Volumes/KOBOeReader` noch einmal laufen lassen und die Ausgabe zu den Ergebnissen legen.
- Pfad `.adds/NickelInkSync/ignore-kobalt` darf am Ende nicht existieren (außer du willst Kobalt daneben).
