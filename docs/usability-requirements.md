# Usability and accessibility requirements for the desktop client

A review of the desktop client (`gui/`, egui/eframe 0.37, macOS and
Windows, German and English) against the ergonomics standards of the
DIN EN ISO 9241 series and against WCAG 2.1 level AA, carried over to
desktop software through WCAG2ICT and EN 301 549 chapter 11. It is the
basis of U1 and U2 in [TODO.md](../TODO.md) (release 0.2.0, accessibility
by 1.0.0 at the latest).

State of the review: 2026-11-02, `main` at `c71a1da`. Code only; no
screen-reader test has been run yet (that is the first U2 step). The
standards are paid documents: everything here is paraphrased from the
publishers' abstracts, tables of contents and freely available secondary
sources listed below, never quoted. This is engineering guidance, not
legal advice.

## Sources

| Standard | What it covers | Use for this program | Source |
| --- | --- | --- | --- |
| DIN EN ISO 9241-110:2020 | Seven interaction principles: suitability for the user's tasks, self-descriptiveness, conformity with user expectations, learnability, controllability, use error robustness, user engagement | The main yardstick for the dialogs (map, ask, restore, copy, settings) and the book list | [ISO 9241-110:2020](https://www.iso.org/standard/75258.html) |
| DIN EN ISO 9241-11:2018 | Usability as effectiveness, efficiency and satisfaction for given users, goals and context of use | Defines what to measure: "bring a book on the Kobo to the KOReader page" without help, in how many steps, with how many errors | [ISO 9241-11:2018](https://www.iso.org/standard/63500.html) |
| DIN EN ISO 9241-112:2017 | Principles for presenting information: detectability, freedom from distraction, discriminability, interpretability, conciseness, consistency | Book groups, status lines, warnings, the server column | [ISO 9241-112:2017](https://www.iso.org/standard/64840.html) |
| DIN EN ISO 9241-125:2017 | Visual presentation of information (successor of 9241-12): legibility, grouping, colour, coding | Contrast, colour-only coding, table layout | [ISO 9241-125:2017](https://www.iso.org/standard/64839.html) |
| DIN EN ISO 9241-143:2012 | Form-based dialogs: labels, input formats, defaults, validation, error feedback | Settings dialog, mapping dialog, copy dialog | [ISO 9241 overview](https://en.wikipedia.org/wiki/ISO_9241) (search "9241-143" in the ISO catalogue) |
| DIN EN ISO 9241-171:2008 | Software accessibility: keyboard access, focus, user preferences, assistive technology, alternatives to colour and sound | Bridge between the 9241 principles and WCAG; covers OS settings WCAG does not name | [ISO 9241-171:2008](https://www.iso.org/standard/39080.html) |
| DIN EN ISO 9241-210:2019 | Human-centred design process: context of use, requirements, design, evaluation with users | How 0.2.0 is run: owner tests per step, as with the device tests | [ISO 9241-210:2019](https://www.iso.org/standard/77520.html) |
| DIN EN 301 549 (V3.2.1, 2021) | Accessibility of ICT; chapter 11 applies WCAG 2.1 A/AA to non-web software and adds platform requirements (11.5 interoperability with assistive technology, 11.7 user preferences) | The checklist for U2 | [ETSI EN 301 549 V3.2.1](https://www.etsi.org/deliver/etsi_en/301500_301599/301549/03.02.01_60/en_301549v030201p.pdf) |
| EN 301 549 V4.1.1 (published 2026-09-02) | Same structure, moved to WCAG 2.2 A/AA; harmonised status depends on its citation in the Official Journal | See decision E2 | [NDA notice](https://nda.ie/news/en301549-published), [Deque summary](https://www.deque.com/blog/en-301-549-v4-1-1-is-final-what-changed-what-it-means-and-what-you-should-do/) |
| WCAG 2.1 (W3C Recommendation) | Success criteria, levels A/AA/AAA | Target level AA | [WCAG 2.1](https://www.w3.org/TR/WCAG21/) |
| WCAG2ICT (W3C Group Note, 2024) | How each WCAG 2.0–2.2 criterion reads for non-web documents and software ("web page" → "software", etc.) | Interpreting the criteria for a desktop app | [WCAG2ICT](https://www.w3.org/TR/wcag2ict-22/), [W3C news](https://www.w3.org/WAI/news/2024-10-08/wcag2ict) |

Legal frame, briefly: the European Accessibility Act (in Germany the
BFSG, in force since 2025-06-28) binds economic operators of certain
consumer products and services; BITV 2.0 binds public bodies. A free,
non-commercial MIT tool is most likely in neither scope, so WCAG 2.1 AA
is a voluntary goal here, as the owner set it.

## What egui/eframe 0.37 brings, and where it stops

Checked in the sources of `egui 0.37.0` and `eframe 0.37.0` (the versions
in `Cargo.lock`).

| Area | What works out of the box | Limits |
| --- | --- | --- |
| Screen readers | eframe's default feature `accesskit` (AccessKit 0.25) is on: buttons, checkboxes, radio buttons, combo boxes, text fields, collapsing headers and windows get roles and names for VoiceOver (macOS) and Narrator/NVDA (Windows, UI Automation) | No table semantics: `egui::Grid` exposes loose labels, not rows, columns and headers. No heading role: `ui.heading` is a label with a bigger font. No live regions: a new notice is not announced. A label next to a field is only its name if the code says `labelled_by` |
| Tooltips | `on_hover_text` also becomes the widget's accessible description (`response.rs` in egui) | Shown on mouse hover only, not on keyboard focus; on a plain label a keyboard user without a screen reader never sees it |
| Keyboard | Tab/Shift-Tab move focus through interactive widgets, Space/Enter activate, arrows in combo boxes; a `Modal` keeps focus inside it and closes on Escape when the code asks `should_close()` | No mnemonics or menu bar, no default button (Enter does not confirm a dialog), no focus return to the opening button after a dialog, no app shortcuts unless coded |
| Scaling | Follows the OS scale factor; `zoom_with_keyboard` is on by default (Cmd/Ctrl `+`/`-`/`0`) | Ignores the Windows "Text size" setting and macOS text-size preferences; zoom is not remembered between starts |
| Theme and contrast | `ThemePreference::System` follows light/dark mode | Ignores Windows high-contrast themes and "increase contrast" (macOS). Several default colours are below 4.5:1 (see A5) |
| Language | Texts follow `sys-locale` (`gui/src/i18n.rs:22`) | AccessKit gets no language tag per node; screen readers use their own voice language |

## Requirements, current state and gaps

Effort: S = an afternoon, M = a few days, L = a week or more. Release:
0.2.0, or "1.0" for "by 1.0.0 at the latest". Line numbers refer to
`main` at `c71a1da`.

### Interaction (ISO 9241-110, -11, -112, -143)

| # | Requirement | Source | Today | Gap | Proposal | Effort | Release |
| --- | --- | --- | --- | --- | --- | --- | --- |
| I1 | The main task is short and obvious: connect, see what differs, send or get | 9241-110 suitability; 9241-11 efficiency | Device list on the left, books grouped by how they sync, server column with outlook, "Send all"/"Get all" (`gui/src/app.rs:1254`–`1301`) | No first-run guidance: without server or library the user sees warnings but no path through the setup | A short first-run checklist above the books (server set? library set? device prepared?), each with its button | S | 0.2.0 |
| I2 | The system tells what a control will do and what state things are in | 9241-110 self-descriptiveness; 9241-112 interpretability | Hints under each group (`app.rs:1311`–`1342`), tooltips on most buttons, "Prepared at …" (`app.rs:1245`) | Map, Unmap, Send and Get in the book rows have no tooltip (`app.rs:1533`–`1574`); the server column's outlook ("asks first") is not explained; percentages hide the real place (XPointer only in a tooltip) | Tooltips on the row buttons naming the book; a legend for the outlook words; keep XPointer in the tooltip (expert detail) | S | 0.2.0 |
| I3 | Same words, places and behaviour throughout; platform conventions | 9241-110 conformity with user expectations; 9241-112 consistency | Every dialog has the action left, Cancel right (`app.rs:741`, `settings_dialog.rs:68`) | No menu bar on macOS (About, Settings, Quit are expected there; Cmd-, for Settings); Windows expects the default button first and Enter to confirm; Overview/History/About are free windows, the rest modals | Native app menu on macOS (via `muda`, or at least Cmd-, and Cmd-Q); Enter = primary action in each modal | M | 0.2.0 |
| I4 | Easy to learn; help where it is needed | 9241-110 learnability | READMEs; hints in the window | No link from the window to help (README section of the client) | "Help" button in the top bar opening the README section in the browser; glossary of "mapped", "reference EPUB", "own checksum" in the README | S | 0.2.0 |
| I5 | The user controls pace and order; can cancel and undo | 9241-110 controllability | Nothing syncs on its own (`no_auto_sync`, `app.rs:548`); restore keeps the state before (`app.rs:354`); push/pull ask per `confirm` | A running push, pull, prepare or copy cannot be cancelled; no undo after "Unmap" (only the question before, `app.rs:1108`); the ask dialog ignores Escape (`app.rs:692`, no `should_close`) | Escape = Cancel in the ask dialog; "Undo" in the notice after Unmap (keep the removed mapping in memory); cancel for copy and prepare (between books) | M | 0.2.0 |
| I6 | Errors are prevented, and when they happen they say what and how to fix | 9241-110 use error robustness; 9241-143 error feedback | Default answer in the ask dialog is "keep" (`app.rs:726`); disabled buttons while busy (`app.rs:627`); errors in red | Library and server errors are passed through as English `e.to_string()` (`gui/src/worker.rs:261`, `gui/src/model.rs:99`) also in the German UI, without a next step; settings are not validated before saving (server URL, library folder) | Map the common errors (server unreachable, wrong password, library not found, device read-only) to bilingual messages with a fix; validate URL and folder in the form | M | 0.2.0 |
| I7 | Feedback for every action, in place and in time | 9241-110 self-descriptiveness/controllability; 9241-112 detectability | Spinner and "n of m" while loading and preparing (`app.rs:600`–`621`); one notice line above the books (`app.rs:591`) | The notice has no time and stays until the next action, so an old result looks current; spinner beside "Send all" without text | Time stamp and a close button on the notice; "Sending 3 books…" next to the spinner | S | 0.2.0 |
| I8 | The program is pleasant and trustworthy to use | 9241-110 user engagement; 9241-11 satisfaction | Release notice, history, about window with licences | Nothing to fix now; measure it | Ask the owner and one more tester after 0.2.0 (short SUS questionnaire) | S | 0.2.0 |
| I9 | Forms: one label per field, clear format, sensible defaults | 9241-143 | Settings in a two-column grid with labels (`gui/src/settings_dialog.rs:101`–`176`); hint texts as examples | Labels are not tied to the fields (`labelled_by` missing, see A2); the password field's state is only a placeholder (`settings_dialog.rs:113`); `confirm` and `position` options have no explanation | Tie labels, add one-line help under `confirm` and `position`; show "stored in the keychain" as text, not only placeholder | S | 0.2.0 |
| I10 | Mapping dialog: best guess pre-selected, the reason visible | 9241-143 defaults; 9241-112 | First candidate pre-selected only if likely (`app.rs:420`–`426`), scores and "uncertain" shown (`app.rs:1052`–`1057`) | Score string is technical | Keep; shorten the score text to "title ✓ author ✓ 12 chapters" style | S | 1.0 |
| I11 | Human-centred process: context of use, requirements, tests with users | 9241-210 | Owner tests every step on devices; decisions recorded in `TODO.md` | No written context of use (who, which devices, how often) and no task-based test | One page "context of use" and three test tasks in this file; run them with the owner before 0.2.0 | S | 0.2.0 |

### Presentation (ISO 9241-112, -125)

| # | Requirement | Source | Today | Gap | Proposal | Effort | Release |
| --- | --- | --- | --- | --- | --- | --- | --- |
| P1 | Information grouped by meaning, most important first | 9241-112; 9241-125 grouping | Books in five groups, warnings first (`app.rs:1311`–`1368`) | The device header holds up to seven status lines (`app.rs:1185`–`1252`) before the first book | Fold library, mappings and write mode into one summary line with a details expander | S | 0.2.0 |
| P2 | Long tables stay readable | 9241-125 | Striped grids, file names shortened with full path in tooltip (`app.rs:1453`) | No sorting or search in the book list; header row scrolls away | Search field above the books (as in the copy dialog); sticky header via `egui_extras::TableBuilder` | M | 1.0 |
| P3 | Colour never the only carrier of meaning | 9241-125 colour coding; WCAG 1.4.1 | Warnings and errors are coloured text with words | Fine today; keep it when adding icons | Rule in the code review | – | – |

### Accessibility (WCAG 2.1 AA via WCAG2ICT and EN 301 549 ch. 11; ISO 9241-171)

| # | Requirement | Source | Today | Gap | Proposal | Effort | Release |
| --- | --- | --- | --- | --- | --- | --- | --- |
| A1 | Everything works with the keyboard alone, with visible focus | WCAG 2.1.1, 2.4.7, 2.4.3; EN 301 549 11.2.1.1; 9241-171 | egui moves focus with Tab and draws a focus ring | Untested; tooltip-only information is out of reach (A3); focus order in the top bar is right-to-left (`app.rs:472`), so Tab runs Language → About → Settings → History → Overview, mirrored to the screen | Audit with keyboard only; build the top bar left-to-right in reading order | S | 0.2.0 |
| A2 | Every control has a name a screen reader reads | WCAG 1.3.1, 4.1.2; EN 301 549 11.4.1.2, 11.5 | Buttons and checkboxes are named by their text | Text fields and combo boxes beside a separate label have no name: settings (`settings_dialog.rs:105`–`172`), language (`app.rs:474`–`485`), copy search (`gui/src/copy_dialog.rs:82`, placeholder only), Linux paths (`app.rs:575`, `app.rs:1169`) | `.labelled_by(label.id)` on each; a small helper `labelled_field()` | S | 0.2.0 |
| A3 | Information in tooltips is reachable without a mouse | WCAG 1.3.1, 1.4.13; 9241-171 | Full paths, XPointers and times are tooltips on labels (`app.rs:1519`, `1580`–`1594`, `976`) | Screen readers get them as description, sighted keyboard users not at all | A details line for the focused or selected book (path, place, server time); make rows selectable | M | 1.0 |
| A4 | Tables read as tables; headings as headings | WCAG 1.3.1; EN 301 549 11.1.3.1 | `egui::Grid` for books, overview, history (`app.rs:1498`, `1396`, `958`) | AccessKit sees loose labels, no rows or headers; headings are plain labels | Short term: give each row one accessible label ("Title, 57 %, server 60 % KOReader, …"). Long term: upstream table/heading roles in egui or `egui_extras` | M (short) / L (upstream) | 0.2.0 / 1.0 |
| A5 | Text contrast at least 4.5:1, UI parts 3:1 | WCAG 1.4.3, 1.4.11; 9241-125 | egui's default themes | Estimated from egui's default colours: weak text (`ui.weak`, used for hints, scores, "prepared at") ≈ 2.7:1 dark and ≈ 2.9:1 light; warning orange on light ≈ 2.8:1; error red ≈ 3.8:1 light, ≈ 4.3:1 dark; hyperlink on light ≈ 2.8:1 | Own `Visuals` for light and dark: `weak_text_color`, `warn_fg_color`, `error_fg_color`, `hyperlink_color` with ≥ 4.5:1; measure from screenshots | S | 0.2.0 |
| A6 | Status messages are announced without moving focus | WCAG 4.1.3 | Notice line, spinners | No live region in egui; notices after push, map, copy are silent for screen readers | Ask AccessKit/egui for live regions (AccessKit has `Live`); until then move focus to the notice after a finished action, or announce via the platform | M | 1.0 |
| A7 | Text can be enlarged to 200 % without loss | WCAG 1.4.4, 1.4.10; 9241-171 user preferences | Cmd/Ctrl `+`/`-` zoom works (egui default) | Not discoverable, not remembered; min window 640×400 (`gui/src/main.rs:41`) gets tight at 200 % | "View" zoom buttons or a setting, store `zoom_factor` in the GUI settings; check every dialog at 200 % | S | 0.2.0 |
| A8 | Respect OS accessibility settings | EN 301 549 11.7; 9241-171 | Light/dark from the OS | Windows high contrast, text size and macOS increase contrast are ignored | Read the high-contrast flag on Windows and switch to a high-contrast `Visuals` | M | 1.0 |
| A9 | Dialogs: focus starts inside and returns afterwards; Escape cancels | WCAG 2.4.3; 9241-171 | Modals trap focus; Escape closes most (`app.rs:801`, `1103`, `1139`) | The ask dialog does not close on Escape (`app.rs:692`); focus not set to the first control on open, not returned to the opener | Request focus on open, remember and restore the opener | S | 0.2.0 |
| A10 | Targets large enough to hit | WCAG 2.5.8 (2.2, AA) | `small_button` in the book rows (`app.rs:1533`, `1541`, `1555`, `1570`) | Small buttons are below 24×24 px | Normal-size buttons in rows (only with E2 = WCAG 2.2) | S | 1.0 |
| A11 | Language of the UI is set and stated | WCAG 3.1.1; EN 301 549 11.3.1.1 | System language or chosen (`i18n.rs:22`) | Library errors stay English (I6) | Covered by I6 | – | – |
| A12 | Help and error messages name the fix | WCAG 3.3.1, 3.3.3 | See I6 | See I6 | See I6 | – | – |
| A13 | Documentation of accessibility features | EN 301 549 12.1.1 | None | No section in the READMEs | "Accessibility" section in README/README.de (keyboard, zoom, screen readers, known limits) | S | 0.2.0 |

## The most important points for 0.2.0

1. **A2 Labels tied to fields** (settings, language, search): the
   cheapest screen-reader fix, S.
2. **A5 Contrast of weak text, warnings, errors and links:** one custom
   `Visuals` for light and dark, S.
3. **A1/A9 Keyboard pass:** top bar in reading order, Escape in the ask
   dialog, Enter for the primary button, focus into and back out of
   dialogs, S–M.
4. **U2 audit** with VoiceOver and Narrator against EN 301 549 chapter
   11, result added to this file (owner on Windows, me on macOS), M.
5. **I6/A12 Understandable errors** in both languages, with the next
   step, M.
6. **I5 Undo and cancel:** undo after Unmap, cancel for copy and prepare,
   M.
7. **I2/I7 Feedback:** tooltips on the row buttons, notice with time and
   close button, spinner with text, S.
8. **A7 Zoom** remembered and visible, all dialogs checked at 200 %, S.
9. **I3 macOS menu bar** with Settings (Cmd-,), About and Quit, M.
10. **A13/I4 Help:** Help button and an accessibility section in the
    READMEs, S.

## Open decisions for the owner

| # | Question | Recommendation |
| --- | --- | --- |
| E1 | Which platforms does the accessibility goal cover: macOS and Windows only, or Linux (AT-SPI, AppImage E8) too? | macOS and Windows for 1.0.0; Linux best effort, since AccessKit supports it but it is not a release target yet |
| E2 | WCAG 2.1 AA as decided, or WCAG 2.2 AA (the basis of EN 301 549 V4.1.1 since September 2026)? | Aim at 2.2 AA: the extra criteria that apply here (2.4.11 focus not obscured, 2.5.8 target size, 3.3.7 redundant entry) are small, and a 1.0.0 in 2027 would otherwise target an outdated version |
| E3 | Gaps egui cannot close (tables, headings, live regions): fix upstream in egui/AccessKit, work around, or accept and document? | Work around in 0.2.0 (row labels, focus on notices), report or contribute upstream, document what is left in the accessibility section; no switch away from egui |
| E4 | Who tests with a screen reader, and is an outside tester (a blind or low-vision user) wanted before 1.0.0? | Owner on Windows (Narrator), me on macOS (VoiceOver) for 0.2.0; one outside test before 1.0.0 |
| E5 | A formal conformance statement (accessibility conformance report, VPAT style) for 1.0.0? | Not formal; a short statement in the READMEs with the level reached and known limits |
| E6 | Menu bar on macOS: real native menu (new dependency `muda`) or shortcuts only? | Shortcuts only in 0.2.0 (no dependency), native menu if testers miss it |
