/*
 * NickelInkSync plugin: pull the reading position from a KOSync server when a
 * sideloaded book is opened, push it when the book is left (Qt 6.5.2; Kobo
 * firmware 6.0.279225 and Tolino Shine Color 5.18.264769).
 *
 * The plugin has no sync logic of its own. It calls the nickelinksync CLI in
 * /mnt/onboard/.adds/NickelInkSync/ with the configuration next to it, which
 * reads the device database (read-only while Nickel runs, `--live`), talks to
 * the server and applies the conflict rule, with the base each book last
 * agreed with the server on and the pending list in state.json
 * (src/state.rs).
 *
 * Opening (ReadingController::setVolume(Volume const&, Bookmark const&)),
 * only for sideloaded books opened without a bookmark (not a link or a
 * search result): before Nickel opens the book, the plugin switches Wi-Fi on
 * (silently, at most kOpenWifiMs) and asks the CLI. How a position to take
 * gets into the book differs:
 *
 *   Kobo: the CLI only decides (`pull --live --nickel-writes --emit`) and
 *   makes the day's backup of the database; the plugin writes the position
 *   through Nickel itself, as the reader Kobalt does (MIT,
 *   github.com/nicoverbruggen/kobalt, platforms/kobo/qt6/native_progress.cpp;
 *   the order of the calls, no code taken over): in one of Nickel's
 *   transactions VolumeManager::getById, updateBookmark, setPercentRead and
 *   updateFields with the changed columns, which also updates Nickel's
 *   Volume cache. In the same transaction it drops the cached pages the CLI
 *   names (NICKELINKSYNC_STALE_PAGES): when none of them holds the place,
 *   Nickel would open at the chapter start on the first opening after a
 *   start (docs/nickel-6.0-symbols.md). Nickel then gets a fresh Volume
 *   from getById, and once the transaction committed the book's base goes
 *   into state.json (NICKELINKSYNC_BASE).
 *
 *   Tolino (until stage C7 decides): the CLI writes the position into the
 *   book's row in the database Nickel holds open (`pull --live --write
 *   --emit`, SQLite locking, one backup a day) and drops those pages; the
 *   plugin hands Nickel a copy of the Volume with that position
 *   (Content::setChapterIdBookmarked, setPercentRead), and sets the same on
 *   the Volume Nickel passed in. Nickel takes the chapter from that Volume
 *   and the place in the chapter from the row it loads again, so they have
 *   to agree (docs/nickel-5.18-symbols.md).
 *
 *   A conflict (both sides moved since the base, NICKELINKSYNC_CONFLICT) is
 *   only reported for now and left alone; the book is not pushed on leaving.
 *
 * Leaving (ReadingView::leavingReadingExperience()):
 *   The original saves the position (saveProgress); after it returns, the CLI
 *   pushes that book in the background (`push --live --emit`, an
 *   asynchronous QProcess whose output goes to the log), once Wi-Fi is up
 *   or kWifiLeaveWaitS passed. Without a network or on a server error the
 *   CLI keeps the book on the pending list (NICKELINKSYNC_PENDING). A book
 *   opened without asking the server (no network) is pushed the same way if
 *   its position moved, else nothing is sent. Leaving never waits for it.
 *   Not on the page the server's position was taken on: Nickel saves that
 *   page's start, which lies before the server's point, so the push would
 *   only ask. Not while the background pull of that book still runs. And a
 *   book Nickel opened elsewhere than at the pulled position gets
 *   `not_in_line` in state.json: the CLI does not push it and takes the
 *   server's position on the next pull.
 *
 * Pending list: whenever the server answered (a pull on opening, a push on
 * leaving) and state.json holds pending books, `sync --pending --live --emit`
 * sends them in the background.
 *
 * Not prepared: without the library index from `nickelinksync device
 * prepare` (on the computer) the CLI sends nothing (NICKELINKSYNC_NO_INDEX);
 * the plugin says so once per start. A kepub without a mapping never syncs.
 *
 * Network: Nickel switches Wi-Fi off when idle. The plugin asks it to connect
 * without a dialog (WirelessWorkflowManager::connectWireless(false, true),
 * which is what connectWirelessSilently() calls). Opening waits for it in a
 * local event loop, so the UI keeps running; leaving does not wait.
 *
 * Messages: each outcome is shown as one of Nickel's toasts
 * (MainWindowController::toast), German only for now; `notify` in config.ini
 * picks which: `all` (default), `errors` (only what went wrong or was left
 * undone) or `none`. The CLI names the outcome in `NICKELINKSYNC_…` lines.
 *
 * Kobalt: while the reader Kobalt is installed, every entry point only calls
 * the originals: Kobalt writes the reading positions into Nickel's database
 * itself and may sync with KOSync itself. Checked in init() and whenever a
 * book is opened or left or the pending list is sent, as src/kobalt.rs
 * decides on the device: its library in Nickel's plugin folder (or moved
 * aside by NickelHook for the first seconds after a start), its file
 * `uninstall` and no `uninstall-now`. Once found, it stays found until
 * Nickel restarts, the only time Kobalt installs or removes itself. One
 * toast the first time, 20 s after the start or when a book is opened in
 * Nickel's reader (with Kobalt as the default reader Nickel's own is not
 * used); `ignore-kobalt` next to the CLI syncs anyway (tests).
 *
 * Symbols and signatures: docs/nickel-6.0-symbols.md (Kobo 6.0, the write
 * path) and docs/nickel-5.18-symbols.md (Tolino 5.18).
 *
 * Inactive without the CLI or config.ini, and on a Kobo without Nickel's
 * write path. Log: NickelInkSync.log next to the CLI.
 */

#include <NickelHook.h>

#include <QByteArray>
#include <QCoreApplication>
#include <QDateTime>
#include <QEventLoop>
#include <QFile>
#include <QFileInfo>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLocale>
#include <QMap>
#include <QProcess>
#include <QProcessEnvironment>
#include <QSqlDatabase>
#include <QSqlError>
#include <QSqlQuery>
#include <QString>
#include <QVariant>
#include <QStringList>
#include <QTimer>

#include <cstdio>
#include <unistd.h>

typedef void Device;
typedef void Volume;
typedef void Bookmark;
typedef void ReadingController;
typedef void ReadingView;
typedef void WirelessWorkflowManager;
typedef void MainWindowController;
typedef void ContentLocation;
typedef void Repository;

static const char kDir[]    = "/mnt/onboard/.adds/NickelInkSync";
static const char kCli[]    = "/mnt/onboard/.adds/NickelInkSync/nickelinksync";
static const char kConfig[] = "/mnt/onboard/.adds/NickelInkSync/config.ini";
static const char kLog[]    = "/mnt/onboard/.adds/NickelInkSync/NickelInkSync.log";
static const char kBackups[] = "/mnt/onboard/.adds/NickelInkSync/backups";
// Per book its base, `not_in_line` (where Nickel opened instead of the
// pulled position: never pushed, the next pull takes the server's) and
// `pending` (src/state.rs). The CLI writes it too; both re-read it and
// change only their own book, then replace it (tmp file and rename).
static const char kState[] = "/mnt/onboard/.adds/NickelInkSync/state.json";
static const int kStateVersion = 1;
// Kobalt (src/kobalt.rs): installed while its library is in place and
// `uninstall` exists without `uninstall-now`. With kIgnoreKobalt the plugin
// syncs anyway; kKobaltNoticed marks that the toast about it was shown.
static const char kKobaltLibrary[] = "/usr/local/Kobo/imageformats/libkobalt.so";
static const char kKobaltFailsafe[] = "/usr/local/Kobo/libkobalt.so.failsafe";
static const char kKobaltKeep[] = "/mnt/onboard/.adds/kobalt/uninstall";
static const char kKobaltRemove[] = "/mnt/onboard/.adds/kobalt/uninstall-now";
static const char kIgnoreKobalt[] = "/mnt/onboard/.adds/NickelInkSync/ignore-kobalt";
static const char kKobaltNoticed[] = "/mnt/onboard/.adds/NickelInkSync/kobalt-noticed";
static const char kOnboard[] = "/mnt/onboard";
static const char kSideloaded[] = "file:///mnt/onboard/";
static const qint64 kLogMaxBytes = 128 * 1024;
// Seconds per request for pull, and how long opening may wait for it as a
// whole (CLI start, database, backup, hash, two requests).
static const char kPullTimeout[] = "5";
static const int kPullWaitMs = 15000;
// How long opening waits for Wi-Fi (Nickel's own check is slow to report
// it, so the CLI tries anyway); how long leaving waits in the background
// before it pushes anyway, which keeps the book pending without a network.
static const int kOpenWifiMs = 3000;
static const int kWifiLeaveWaitS = 15;
// A Volume is 12 bytes in 5.18 (vtable, shared data, flags; see the copy
// constructor), a Bookmark 8 (vtable, shared data). Room to spare.
static const size_t kVolumeBytes = 64;
static const size_t kBookmarkBytes = 64;
// When to check that Nickel opened the pulled chapter. Nickel may not know
// its place yet after 3 s (an empty location, in 8 of 9 runs of a device
// test), so the check asks again every second, kCheckOpenTries times, up to
// 15 s in all. Since stale cached pages are dropped (PR #48) Nickel renders
// the chapter fresh and knew its place only after 8 s in a device test
// (31.10.), close to the old limit of 11 s; a bigger book takes longer.
static const int kCheckOpenMs = 3000;
static const int kCheckOpenRetryMs = 1000;
static const int kCheckOpenTries = 12;
// Toasts: how long they stay (Nickel's own use 1500 ms), and how long after
// setVolume the one about opening is queued, once the reading view is up.
static const int kToastMs = 5000;
static const int kProblemToastMs = 8000;
static const int kOpenToastDelayMs = 1000;
// How long after init() the toast about Kobalt is shown: Nickel's home
// screen is up by then.
static const int kKobaltToastDelayMs = 20000;
// Seconds per request for push and the pending list, and when the plugin
// gives up on them.
static const char kPushTimeout[] = "15";
static const int kPushWaitMs = 60000;
// How long after opening the pending list is sent: the book is drawn by then.
static const int kPendingDelayMs = 5000;

// Originals, filled by NickelHook (.out).
static void (*ReadingController__setVolume)(ReadingController *, const Volume *, const Bookmark *);
static void (*ReadingView__leavingReadingExperience)(ReadingView *);

// Resolved via dlsym.
static Device *(*Device__getCurrentDevice)();  // returns Device& (r0)
static bool (*Device__isTolino)(const Device *);
// QString is returned through a hidden pointer (r0), `this` follows in r1;
// declaring the return type as QString gives exactly that ABI. The argument
// is unused by 5.18.
static QString (*Content__getId)(const Volume *, const QString &);
static bool (*Bookmark__isNull)(const Bookmark *);
static void (*Volume__copy)(Volume *, const Volume *);  // Volume(Volume const&)
static void (*Volume__dtor)(Volume *);
static void (*Content__setChapterIdBookmarked)(Volume *, const QVariant &);
static void (*Content__setPercentRead)(Volume *, const QVariant &);
static MainWindowController *(*MainWindowController__sharedInstance)();
// Returns a QWidget*; QObject is its first base, so the pointer is the same.
static QObject *(*MainWindowController__currentView)(const MainWindowController *);
// Bookmark by value: hidden result pointer in r0, `this` in r1.
static void (*ReadingView__bookmarkForCurrentLocation)(void *result, ReadingView *);
// Returns a reference (d + 0x104), not a value.
static const ContentLocation *(*Bookmark__getStartLocation)(const Bookmark *);
static QString (*ContentLocation__toString)(const ContentLocation *);
static void (*Bookmark__dtor)(Bookmark *);

// Optional (NULL if missing): without them the plugin tries without Wi-Fi.
static WirelessWorkflowManager *(*WirelessWorkflowManager__sharedInstance)();
static bool (*WirelessWorkflowManager__isInternetAccessible)(WirelessWorkflowManager *);
static void (*WirelessWorkflowManager__connectWireless)(WirelessWorkflowManager *, bool, bool);
// Optional: without it, no messages. Queues a toast with a primary and a
// secondary line for `ms` milliseconds; returns nothing (a tail call to
// N3ToasterController::queueMeSomeToast, see docs/nickel-5.18-symbols.md).
static void (*MainWindowController__toast)(MainWindowController *, const QString &, const QString &, int);
// Optional, for the log only: the book's row as Nickel holds it.
// VolumeManager::getById is static and returns the Volume by value (hidden
// result pointer in r0); Content::getDbValues returns the columns by value.
static void (*VolumeManager__getById)(Volume *result, const QString &id, const QString &db);
static bool (*Volume__isValid)(const Volume *);
static QMap<QString, QVariant> (*Content__getDbValues)(const Volume *);
// Optional, required on a Kobo: Nickel's own write path
// (docs/nickel-6.0-symbols.md, "Kobalt's write path"). getDbName returns a
// reference; ContentLocation::create returns its 8 bytes by value (hidden
// result pointer in r0) and takes both strings by value.
static const QString &(*Device__getDbName)(const Device *);
static bool (*DatabaseAdapter__beginTransaction)(QString);
static bool (*DatabaseAdapter__endTransaction)(QString, bool);
static QSqlDatabase (*DatabaseAdapter__dbConnection)(const QString &, bool);
static QMap<QString, QVariant> (*Volume__getDbValues)(const Volume *);
static void (*ContentLocation__create)(ContentLocation *result, QString position, QString spine);
static bool (*ContentLocation__isValid)(const ContentLocation *);
static void (*ContentLocation__dtor)(ContentLocation *);
static void (*VolumeManager__updateBookmark)(Volume *, const QString &, int, const QDateTime &, int);
static bool (*VolumeManager__updateFields)(const QString &, const QMap<QString, QVariant> &,
                                           const QMap<QString, QVariant> &, const QString &);
static Repository *(*Repository__refreshCache_Volume)(const QString &);

// `notify` in config.ini.
enum class Notify { All, Errors, None };
static Notify g_notify = Notify::All;

// Set at the end of a successful init(); every hook passes through until then.
static bool g_active = false;
// A Kobo: positions are written through Nickel (a Tolino: by the CLI).
static bool g_kobo = false;
// Kobalt was found installed: every hook passes through until Nickel
// restarts.
static bool g_kobalt = false;
// ContentID of the book opened last (GUI thread only).
static QString g_open_id;
// The book whose position needs an answer (a conflict) or cannot be taken
// from the server, or that was not brought in line with it: not pushed.
static QString g_not_synced;
// Why, for the message on leaving.
static QString g_not_synced_why;
// The book opened without asking the server (no network, a server error)
// and the page Nickel opened it on (its start, empty until known): pushed
// on leaving only if the reader left that page. Nickel's row is no guide:
// on a book's first opening it saves the first page on leaving anyway.
static QString g_unasked_id;
static QString g_unasked_at;
// The toast about a device without `device prepare` was shown.
static bool g_no_index_noticed = false;
// The page Nickel opened a book on after taking the server's position
// (its start, as Nickel saves it), and that book. Leaving on that page
// sends nothing: the page start lies before the server's point, and the
// push would ask (confirm = backwards) although nothing was read.
static QString g_taken_id;
static QString g_taken_page;

static void log_line(const QString &msg)
{
    nh_log("%s", qPrintable(msg));
    QFile f(QString::fromLatin1(kLog));
    if (f.exists() && f.size() > kLogMaxBytes) {
        f.remove();
    }
    if (f.open(QIODevice::WriteOnly | QIODevice::Append | QIODevice::Text)) {
        f.write(QDateTime::currentDateTime().toString(Qt::ISODate).toUtf8());
        f.write(" ");
        f.write(msg.toUtf8());
        f.write("\n");
    }
}

// Reads `notify` from config.ini (the CLI checks the value too); `all`
// if it is missing or not understood.
static Notify read_notify()
{
    QFile f(QString::fromLatin1(kConfig));
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return Notify::All;
    }
    Notify notify = Notify::All;
    for (const QByteArray &raw : f.readAll().split('\n')) {
        const QByteArray line = raw.trimmed();
        const int eq = line.indexOf('=');
        if (line.startsWith('#') || line.startsWith(';') || eq < 0 || line.left(eq).trimmed() != "notify") {
            continue;
        }
        const QByteArray value = line.mid(eq + 1).trimmed();
        if (value == "errors") {
            notify = Notify::Errors;
        } else if (value == "none") {
            notify = Notify::None;
        } else {
            if (value != "all") {
                log_line(QStringLiteral("notify = %1 is not all, errors or none: using all").arg(QString::fromUtf8(value)));
            }
            notify = Notify::All;
        }
    }
    return notify;
}

// The Tolino cuts the primary line after about 24 characters and the
// secondary after about 45 (device test), so both are kept short.
// Shows one of Nickel's toasts after `delay_ms`, if `notify` allows it: a
// `problem` (something went wrong or was left undone) unless `none`, any
// other only with `all`.
static void toast(bool problem, const QString &primary, const QString &secondary, int delay_ms = 0)
{
    if (!MainWindowController__toast || g_notify == Notify::None || (g_notify == Notify::Errors && !problem)) {
        return;
    }
    log_line(QStringLiteral("toast: %1 / %2").arg(primary, secondary));
    QTimer::singleShot(delay_ms, qApp, [problem, primary, secondary] {
        MainWindowController__toast(MainWindowController__sharedInstance(), primary, secondary,
                                    problem ? kProblemToastMs : kToastMs);
    });
}

// The fields of each `NICKELINKSYNC_<name>` line in the CLI's output.
static QList<QList<QByteArray>> machine_lines(const QByteArray &out, const char *name)
{
    const QByteArray prefix = QByteArray("NICKELINKSYNC_") + name;
    QList<QList<QByteArray>> found;
    for (const QByteArray &line : out.split('\n')) {
        QList<QByteArray> f = line.trimmed().split('\t');
        if (f[0] == prefix) {
            f.removeFirst();
            found << f;
        }
    }
    return found;
}

// Why the server could not be asked, from `NICKELINKSYNC_ERROR<tab>kind<tab>
// message`, for the secondary line.
static QString error_reason(const QByteArray &out)
{
    const QList<QList<QByteArray>> errors = machine_lines(out, "ERROR");
    const QByteArray kind = errors.isEmpty() ? QByteArray() : errors[0].value(0);
    if (kind == "timeout") {
        return QStringLiteral("Server antwortet nicht");
    }
    if (kind == "network") {
        return QStringLiteral("Keine Verbindung zum Server");
    }
    if (kind == "auth") {
        return QStringLiteral("Anmeldung abgelehnt");
    }
    if (kind == "server") {
        return QStringLiteral("Unerwartete Antwort vom Server");
    }
    return QStringLiteral("Fehler, siehe NickelInkSync.log");
}

// Why a position was not taken or sent, from the reason field of
// `NICKELINKSYNC_NOT_TAKEN` / `NICKELINKSYNC_NOT_SENT`.
static QString skip_reason(const QByteArray &reason)
{
    if (reason == "server-newer") {
        return QStringLiteral("Server hat einen neueren Stand");
    }
    if (reason == "not-in-line") {
        return QStringLiteral("Beim Öffnen nicht abgeglichen");
    }
    if (reason == "not-translated") {
        return QStringLiteral("Position passt nicht zum Buch");
    }
    // Only an original from the library index (`device prepare`) or a
    // mapped book syncs; a kepub always needs a mapping.
    if (reason == "not-mapped") {
        return QStringLiteral("Buch nicht zugeordnet");
    }
    if (reason == "no-table") {
        return QStringLiteral("Zuordnung nicht vorbereitet");
    }
    return QStringLiteral("Buch oder Position nicht lesbar");
}

static const QString kUnansweredReason = QStringLiteral("Braucht Bestätigung (confirm)");

static bool exists(const char *path)
{
    return QFileInfo::exists(QString::fromLatin1(path));
}

// Kobalt is installed, as src/kobalt.rs decides on the device.
static bool kobalt_installed()
{
    return (exists(kKobaltLibrary) || exists(kKobaltFailsafe)) && exists(kKobaltKeep) && !exists(kKobaltRemove);
}

// Whether Kobalt stops the plugin. Once it does, it does until Nickel
// restarts: Kobalt installs and removes itself only when Nickel starts, so
// a deleted `uninstall` leaves it running until then.
static bool kobalt_blocks()
{
    if (g_kobalt) {
        return true;
    }
    if (!kobalt_installed()) {
        return false;
    }
    if (exists(kIgnoreKobalt)) {
        log_line(QStringLiteral("Kobalt is installed, but %1 exists: syncing anyway").arg(QString::fromLatin1(kIgnoreKobalt)));
        return false;
    }
    log_line(QStringLiteral("inactive: Kobalt is installed (%1); it manages the reading positions itself. "
                            "Nothing is written or sent until Nickel restarts without it")
                 .arg(QString::fromLatin1(exists(kKobaltLibrary) ? kKobaltLibrary : kKobaltFailsafe)));
    g_kobalt = true;
    return true;
}

// The toast about Kobalt after `delay_ms`, only the first time it is found
// (kKobaltNoticed; init() removes the mark once Kobalt is gone).
static void notice_kobalt(int delay_ms)
{
    QFile mark(QString::fromLatin1(kKobaltNoticed));
    if (mark.exists() || !mark.open(QIODevice::WriteOnly)) {
        return;
    }
    mark.close();
    toast(true, QStringLiteral("NickelInkSync ist aus"), QStringLiteral("Kobalt ist installiert, siehe README"),
          delay_ms);
}

static bool online()
{
    return WirelessWorkflowManager__sharedInstance && WirelessWorkflowManager__isInternetAccessible
        && WirelessWorkflowManager__isInternetAccessible(WirelessWorkflowManager__sharedInstance());
}

// Asks Nickel to connect, from the event loop: `silent` to a known network
// without a dialog, otherwise with Nickel's Wi-Fi dialog, which shows the
// network list when it cannot connect at once (device test).
static bool connect_wifi(bool silent)
{
    if (!WirelessWorkflowManager__sharedInstance || !WirelessWorkflowManager__connectWireless) {
        return false;
    }
    QTimer::singleShot(0, qApp, [silent] {
        WirelessWorkflowManager__connectWireless(WirelessWorkflowManager__sharedInstance(), false, silent);
    });
    return true;
}

// Waits until the network is there, at most `ms`, in a local event loop so
// the UI keeps running; connects silently (no dialog over the book).
static bool wait_online(int ms)
{
    if (online()) {
        return true;
    }
    if (!connect_wifi(true)) {
        return false;
    }
    log_line(QStringLiteral("no network: switching Wi-Fi on, waiting up to %1 s").arg(ms / 1000));
    QEventLoop loop;
    QTimer poll;
    QObject::connect(&poll, &QTimer::timeout, &loop, [&loop] {
        if (online()) {
            loop.quit();
        }
    });
    QTimer::singleShot(ms, &loop, &QEventLoop::quit);
    poll.start(500);
    loop.exec();
    const bool ok = online();
    log_line(ok ? QStringLiteral("network is there") : QStringLiteral("still no network"));
    return ok;
}

// Runs `fn` once the network is there, polling once a second for at most
// `seconds` without waiting, and then anyway: the CLI keeps the book
// pending when it cannot reach the server. Connects silently, no dialog.
template <typename F>
static void when_online(int seconds, F fn)
{
    if (online() || !connect_wifi(true)) {
        fn();
        return;
    }
    log_line(QStringLiteral("no network: switching Wi-Fi on, waiting up to %1 s").arg(seconds));
    QTimer *t = new QTimer(qApp);
    int n = 0;  // C++11 for the Qt 5 build: no init-capture
    QObject::connect(t, &QTimer::timeout, t, [t, fn, seconds, n]() mutable {
        if (online()) {
            t->deleteLater();
            log_line(QStringLiteral("network is there after %1 s").arg(n + 1));
            fn();
        } else if (++n >= seconds) {
            t->deleteLater();
            log_line(QStringLiteral("still no network after %1 s: trying anyway").arg(seconds));
            fn();
        }
    });
    t->start(1000);
}

// The reading view, if it is the current view.
static ReadingView *reading_view()
{
    QObject *view = MainWindowController__currentView(MainWindowController__sharedInstance());
    return view && view->objectName() == QLatin1String("ReadingView") ? view : nullptr;
}

// Where Nickel stands in the open book, or an empty string.
static QString current_location(ReadingView *view)
{
    alignas(8) unsigned char bookmark[kBookmarkBytes];
    ReadingView__bookmarkForCurrentLocation(bookmark, view);
    const QString at = ContentLocation__toString(Bookmark__getStartLocation(bookmark));
    Bookmark__dtor(bookmark);
    return at;
}

// The chapter file of a position (`OEBPS/text/chapter1.xhtml#point(...)`).
static QString chapter_of(const QString &position)
{
    return position.section(QLatin1Char('#'), 0, 0);
}

// Reads state.json; an empty object if it is missing. False if it cannot
// be read or is newer than this plugin knows (the CLI refuses it too).
static bool read_state(QJsonObject *root)
{
    QFile f(QString::fromLatin1(kState));
    if (!f.exists()) {
        *root = QJsonObject();
        return true;
    }
    if (!f.open(QIODevice::ReadOnly)) {
        log_line(QStringLiteral("could not read %1: %2").arg(QString::fromLatin1(kState), f.errorString()));
        return false;
    }
    QJsonParseError error;
    const QJsonDocument doc = QJsonDocument::fromJson(f.readAll(), &error);
    if (error.error != QJsonParseError::NoError || !doc.isObject()) {
        log_line(QStringLiteral("could not read %1: %2").arg(QString::fromLatin1(kState), error.errorString()));
        return false;
    }
    *root = doc.object();
    if (root->value(QStringLiteral("version")).toInt() > kStateVersion) {
        log_line(QStringLiteral("%1 has a newer format: left alone").arg(QString::fromLatin1(kState)));
        return false;
    }
    return true;
}

// Changes the entry of one book in state.json as the CLI does
// (src/state.rs): read it again, change only this book (an entry without
// base, not_in_line and pending goes), write a tmp file and rename it.
template <typename F>
static bool update_state(const QString &id, F change)
{
    QJsonObject root;
    if (!read_state(&root)) {
        return false;
    }
    QJsonObject books = root.value(QStringLiteral("books")).toObject();
    QJsonObject entry = books.value(id).toObject();
    change(entry);
    if (entry.contains(QStringLiteral("base")) || entry.contains(QStringLiteral("not_in_line"))
        || entry.contains(QStringLiteral("pending"))) {
        books.insert(id, entry);
    } else {
        books.remove(id);
    }
    root.insert(QStringLiteral("version"), kStateVersion);
    root.insert(QStringLiteral("books"), books);
    const QString tmp = QStringLiteral("%1/.state.json.%2.tmp").arg(QString::fromLatin1(kDir)).arg(getpid());
    QFile f(tmp);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)
        || f.write(QJsonDocument(root).toJson(QJsonDocument::Indented)) < 0 || !f.flush()) {
        log_line(QStringLiteral("could not write %1: %2").arg(tmp, f.errorString()));
        f.remove();
        return false;
    }
    fsync(f.handle());
    f.close();
    if (std::rename(QFile::encodeName(tmp).constData(), kState) != 0) {
        log_line(QStringLiteral("could not replace %1").arg(QString::fromLatin1(kState)));
        QFile::remove(tmp);
        return false;
    }
    return true;
}

// Puts a book on the not-in-line list with the place Nickel opened it at,
// or with an empty `at` takes it off.
static void set_not_in_line(const QString &id, const QString &at)
{
    update_state(id, [&at](QJsonObject &entry) {
        if (at.isEmpty()) {
            entry.remove(QStringLiteral("not_in_line"));
        } else {
            entry.insert(QStringLiteral("not_in_line"), at);
        }
    });
}

// Nickel committed the position the CLI chose: its base (the CLI's
// NICKELINKSYNC_BASE object) is what the book now agrees with the server
// on. As in the CLI, that ends not_in_line and pending.
static bool set_base(const QString &id, const QJsonObject &base)
{
    return update_state(id, [&base](QJsonObject &entry) {
        entry.insert(QStringLiteral("base"), base);
        entry.remove(QStringLiteral("not_in_line"));
        entry.remove(QStringLiteral("pending"));
    });
}

// Whether state.json holds a book to send.
static bool has_pending()
{
    QJsonObject root;
    if (!read_state(&root)) {
        return false;
    }
    const QJsonObject books = root.value(QStringLiteral("books")).toObject();
    for (auto it = books.constBegin(); it != books.constEnd(); ++it) {
        if (it.value().toObject().contains(QStringLiteral("pending"))) {
            return true;
        }
    }
    return false;
}

// The node path of a position (`…#point(/1/4/100/1:101)` → 1, 4, 100, 1).
static QList<int> point_path(const QString &position)
{
    QList<int> path;
    const QString point = position.section(QStringLiteral("#point("), 1).section(QLatin1Char(':'), 0, 0);
    for (const QString &step : point.split(QLatin1Char('/'), Qt::SkipEmptyParts)) {
        path << step.toInt();
    }
    return path;
}

// Most node-index steps between the start of the page Nickel shows and the
// pulled point on it: Nickel counts every node, two per paragraph, so 12 is
// about 6 paragraphs. In the device tests the page Nickel opened on started
// 2 to 4 steps before the point; 40 let the chapter start (`/1/4/2/1:0`)
// pass for a point at `/1/4/38`.
static const int kNearSteps = 12;

// Whether `at` (the start of the page Nickel shows) can be the page with
// `target`: the same chapter, not after it, and at most kNearSteps nodes
// before it where the paths first differ below the body.
static bool near(const QString &at, const QString &target)
{
    if (chapter_of(at) != chapter_of(target)) {
        return false;
    }
    const QList<int> a = point_path(at), t = point_path(target);
    if (a.isEmpty() || t.isEmpty()) {
        return true;  // not a point: the chapter has to do
    }
    for (int i = 0; i < qMin(a.size(), t.size()); ++i) {
        if (a[i] != t[i]) {
            return i >= 2 && a[i] < t[i] && (i > 2 || t[i] - a[i] <= kNearSteps);
        }
    }
    return true;
}

// The book is in line with the server: shown as taken, and pushed on
// leaving unless Nickel is still on page `at`.
static void taken_from_server(const QString &id, const QString &at, const QString &taken)
{
    if (g_not_synced == id) {
        g_not_synced.clear();
    }
    g_taken_id = id;
    g_taken_page = at;
    set_not_in_line(id, QString());
    toast(false, QStringLiteral("Fortschritt übernommen"), taken);
}

// After opening: Nickel shows a page start, which lies before the pulled
// point, so "reached" means near it in the same chapter. Only then is the
// book in line with the server and `taken` shown. An empty location means
// Nickel does not know its place yet: asked again, `tries` more times.
static void check_open(const QString &id, const QString &target, const QString &taken, int tries)
{
    ReadingView *view = reading_view();
    if (!view || g_open_id != id) {
        log_line(QStringLiteral("after opening: not in the reading view of the book"));
        return;
    }
    const QString at = current_location(view);
    const int waited_ms = kCheckOpenMs + (kCheckOpenTries - tries) * kCheckOpenRetryMs;
    if (at.isEmpty() && tries > 0) {
        QTimer::singleShot(kCheckOpenRetryMs, qApp, [id, target, taken, tries] {
            check_open(id, target, taken, tries - 1);
        });
        return;
    }
    log_line(QStringLiteral("after opening (%1 ms): Nickel is at %2, pulled %3").arg(waited_ms).arg(at, target));
    if (near(at, target)) {
        log_line(QStringLiteral("after opening: Nickel is at %1, on the page with the pulled point").arg(at));
        taken_from_server(id, at, taken);
    } else if (chapter_of(at) == chapter_of(target)) {
        log_line(QStringLiteral("after opening: Nickel is at %1, in the pulled chapter but not near the point; "
                                "not pushed until a pull brings it in line").arg(at));
        set_not_in_line(id, at);
        toast(true, QStringLiteral("Nicht übernommen"),
              QStringLiteral("Andere Stelle im Kapitel geöffnet"));
    } else {
        log_line(QStringLiteral("after opening: Nickel is at %1, not in the pulled chapter; "
                                "not pushed until a pull brings it in line").arg(at));
        set_not_in_line(id, at);
        toast(true, QStringLiteral("Nicht übernommen"),
              QStringLiteral("Anderes Kapitel geöffnet"));
    }
}

// After opening a book without the server: notes the page Nickel shows,
// asking again while Nickel does not know it yet (as check_open()).
static void note_opened_page(const QString &id, int tries)
{
    ReadingView *view = reading_view();
    if (!view || g_open_id != id || g_unasked_id != id) {
        return;
    }
    const QString at = current_location(view);
    if (at.isEmpty() && tries > 0) {
        QTimer::singleShot(kCheckOpenRetryMs, qApp, [id, tries] { note_opened_page(id, tries - 1); });
        return;
    }
    g_unasked_at = at;
    log_line(QStringLiteral("opened without the server on %1").arg(at));
}

// The row of a Volume as Nickel holds it, for the log (empty without
// Content::getDbValues).
static QMap<QString, QVariant> row_of(const Volume *volume)
{
    return Content__getDbValues ? Content__getDbValues(volume) : QMap<QString, QVariant>();
}

static QString short_value(const QVariant &value)
{
    const QString text = value.toString();
    return text.size() > 80 ? text.left(80) + QStringLiteral("…") : text;
}

// Every column with a value, `name=value; …`.
static QString row_text(const QMap<QString, QVariant> &row)
{
    QStringList fields;
    for (auto it = row.constBegin(); it != row.constEnd(); ++it) {
        if (!it.value().toString().isEmpty()) {
            fields << it.key() + QLatin1Char('=') + short_value(it.value());
        }
    }
    return fields.join(QStringLiteral("; "));
}

// The columns that differ, `name: old -> new; …`, or "none".
static QString row_diff(const QMap<QString, QVariant> &before, const QMap<QString, QVariant> &after)
{
    QStringList keys = before.keys() + after.keys();
    keys.removeDuplicates();
    keys.sort();
    QStringList changed;
    for (const QString &key : keys) {
        if (before.value(key).toString() != after.value(key).toString()) {
            changed << QStringLiteral("%1: %2 -> %3").arg(key, short_value(before.value(key)), short_value(after.value(key)));
        }
    }
    return changed.isEmpty() ? QStringLiteral("none") : changed.join(QStringLiteral("; "));
}

// The row Nickel opened the current book with; leaving logs what Nickel
// changed in it.
static QMap<QString, QVariant> g_open_row;

// The common arguments for one book: device, configuration, book, live.
static QStringList book_args(const char *command, const QString &id)
{
    QStringList args{
        QString::fromLatin1(command),
        QStringLiteral("--live"),
        QStringLiteral("--device"), QString::fromLatin1(kOnboard),
        QStringLiteral("--config"), QString::fromLatin1(kConfig),
    };
    if (!id.isEmpty()) {
        args << QStringLiteral("--content-id") << id;
    }
    return args;
}

// A device without `device prepare`: the CLI sends nothing. Said once per
// start (the log has every time).
static void notice_no_index(const QByteArray &out)
{
    const QList<QList<QByteArray>> lines = machine_lines(out, "NO_INDEX");
    log_line(QStringLiteral("not prepared: %1").arg(QString::fromUtf8(lines.value(0).value(0))));
    if (g_no_index_noticed) {
        return;
    }
    g_no_index_noticed = true;
    toast(true, QStringLiteral("Gerät nicht vorbereitet"), QStringLiteral("Am Rechner „Gerät vorbereiten“"));
}

// NICKELINKSYNC_CONFLICT (ContentID, why, device percent, time, position,
// server percent, time, device, XPointer), for the log; the dialog that
// answers it comes later (stage C5).
static void log_conflicts(const QByteArray &out)
{
    for (const QList<QByteArray> &f : machine_lines(out, "CONFLICT")) {
        log_line(QStringLiteral("conflict (%1), left alone: device %2 % at %3, server %4 % from %5")
                     .arg(QString::fromUtf8(f.value(1)), QString::fromUtf8(f.value(2)), QString::fromUtf8(f.value(4)),
                          QString::fromUtf8(f.value(5)), QString::fromUtf8(f.value(7))));
    }
}

struct Pulled {
    // The CLI ran and finished with exit 0 (the server answered).
    bool ok = false;
    // Why not, for the message, and the kind from NICKELINKSYNC_ERROR.
    QString error;
    QByteArray error_kind;
    // A position would need an answer the plugin cannot give.
    bool unanswered = false;
    // Both sides moved since the base (NICKELINKSYNC_CONFLICT).
    bool conflict = false;
    // The server's position cannot be taken (NICKELINKSYNC_NOT_TAKEN): why.
    QByteArray not_taken;
    bool found = false;
    int percent = 0;
    QByteArray position;
    // Who stored it on the server.
    QString from;
    // Kobo (--nickel-writes): DateLastRead to set (empty: keep), the cached
    // pages to drop, and the base to keep once Nickel committed.
    QString last_read;
    QStringList stale_pages;
    QJsonObject base;
};

// Runs the pull for one book and waits for it: on a Kobo `pull --live
// --nickel-writes --emit`, which only decides and makes the day's backup,
// on a Tolino `pull --live --write --emit`, which also writes. Both print
// a position to take as `NICKELINKSYNC_POSITION<tab>percent<tab>position`.
// Found only if the CLI finished with exit 0.
static Pulled pull(const QString &id)
{
    Pulled result;
    QStringList args = book_args("pull", id);
    args << (g_kobo ? QStringLiteral("--nickel-writes") : QStringLiteral("--write")) << QStringLiteral("--emit")
         << QStringLiteral("--timeout") << QString::fromLatin1(kPullTimeout);
    QProcess p;
    QProcessEnvironment env = QProcessEnvironment::systemEnvironment();
    env.insert(QStringLiteral("NICKELINKSYNC_BACKUP_DIR"), QString::fromLatin1(kBackups));
    p.setProcessEnvironment(env);
    p.setProcessChannelMode(QProcess::MergedChannels);
    p.start(QString::fromLatin1(kCli), args);
    if (!p.waitForStarted(3000)) {
        log_line(QStringLiteral("pull: could not start the CLI: %1").arg(p.errorString()));
        result.error = QStringLiteral("NickelInkSync startet nicht");
        return result;
    }
    if (!p.waitForFinished(kPullWaitMs)) {
        p.kill();
        p.waitForFinished(1000);
        log_line(QStringLiteral("pull: no answer within %1 s, book opens unchanged").arg(kPullWaitMs / 1000));
        result.error = QStringLiteral("Keine Antwort innerhalb von %1 s").arg(kPullWaitMs / 1000);
        result.error_kind = "timeout";
        return result;
    }
    const QByteArray out = p.readAll();
    log_line(QStringLiteral("pull %1 (exit %2):\n%3").arg(id).arg(p.exitCode()).arg(QString::fromUtf8(out).trimmed()));
    if (p.exitStatus() != QProcess::NormalExit || p.exitCode() != 0) {
        result.error = error_reason(out);
        const QList<QList<QByteArray>> errors = machine_lines(out, "ERROR");
        if (!errors.isEmpty()) {
            result.error_kind = errors[0].value(0);
        }
        return result;
    }
    result.ok = true;
    result.unanswered = !machine_lines(out, "UNANSWERED").isEmpty();
    result.conflict = !machine_lines(out, "CONFLICT").isEmpty();
    log_conflicts(out);
    const QList<QList<QByteArray>> not_taken = machine_lines(out, "NOT_TAKEN");
    if (!not_taken.isEmpty()) {
        result.not_taken = not_taken[0].value(0);
    }
    for (const QList<QByteArray> &f : machine_lines(out, "POSITION")) {
        bool ok = false;
        const int percent = f.value(0).toInt(&ok);
        if (ok && f.size() == 2 && !f[1].isEmpty()) {
            result.found = true;
            result.percent = percent;
            result.position = f[1];
        }
    }
    const QList<QList<QByteArray>> from = machine_lines(out, "FROM");
    if (!from.isEmpty()) {
        result.from = QString::fromUtf8(from[0].value(0));
    }
    const QList<QList<QByteArray>> last_read = machine_lines(out, "LAST_READ");
    if (!last_read.isEmpty()) {
        result.last_read = QString::fromUtf8(last_read[0].value(0));
    }
    const QList<QList<QByteArray>> stale = machine_lines(out, "STALE_PAGES");
    for (const QByteArray &page : stale.value(0)) {
        if (!page.isEmpty()) {
            result.stale_pages << QString::fromUtf8(page);
        }
    }
    const QList<QList<QByteArray>> base = machine_lines(out, "BASE");
    if (!base.isEmpty()) {
        result.base = QJsonDocument::fromJson(base[0].value(0)).object();
    }
    if (g_kobo && result.found && stale.isEmpty()) {
        // Nickel's write path needs the CLI's whole hand-over.
        log_line(QStringLiteral("pull: the CLI named no cached pages (too old?): not taken"));
        result.found = false;
        result.not_taken = "other";
    }
    return result;
}

// Kobo: whether Nickel's write path is there (all optional symbols).
static bool can_write_through_nickel()
{
    return Device__getDbName && DatabaseAdapter__beginTransaction && DatabaseAdapter__endTransaction
        && DatabaseAdapter__dbConnection && Volume__getDbValues && ContentLocation__create && ContentLocation__isValid
        && ContentLocation__dtor && VolumeManager__updateBookmark && VolumeManager__updateFields
        && Repository__refreshCache_Volume && VolumeManager__getById && Volume__isValid;
}

// Drops the cached pages `ids` on Nickel's connection, inside its open
// transaction (the rule of drop_stale_pages in src/db.rs, applied by the
// CLI). False on a database error.
static bool drop_pages(const QString &db, const QStringList &ids)
{
    QSqlDatabase conn = DatabaseAdapter__dbConnection(db, false);
    for (const QString &id : ids) {
        for (const char *sql : {"DELETE FROM CachePersistenceRenderedPageChapterData WHERE cachePersistenceRenderedPageDataId = ?",
                                "DELETE FROM CachePersistenceRenderedPageData WHERE id = ?"}) {
            QSqlQuery q(conn);
            q.prepare(QString::fromLatin1(sql));
            q.addBindValue(id);
            if (!q.exec()) {
                log_line(QStringLiteral("dropping cached page %1: %2").arg(id, q.lastError().text()));
                return false;
            }
        }
    }
    return true;
}

// Kobo: writes the pulled position into the book's row through Nickel, in
// one of its transactions, and drops the cached pages that would hide it.
// The order of the calls follows Kobalt (see the top of this file). Rolls
// back and reloads Nickel's Volume cache on any failure; false then.
static bool write_through_nickel(const QString &id, const Pulled &p)
{
    const QString db = Device__getDbName(Device__getCurrentDevice());
    const QString position = QString::fromUtf8(p.position);
    // Nickel has to read the place back as it is.
    alignas(8) unsigned char location[kBookmarkBytes];
    ContentLocation__create(location, position, QString());
    const bool valid = ContentLocation__isValid(location);
    const QString serialized = valid ? ContentLocation__toString(location) : QString();
    ContentLocation__dtor(location);
    if (!valid || serialized != position) {
        log_line(QStringLiteral("write: Nickel does not take %1 as a place (%2)").arg(position, serialized));
        return false;
    }
    if (!DatabaseAdapter__beginTransaction(db)) {
        log_line(QStringLiteral("write: Nickel has a transaction open, nothing written"));
        return false;
    }
    alignas(8) unsigned char volume[kVolumeBytes];
    VolumeManager__getById(volume, id, db);
    bool ok = Volume__isValid(volume);
    QMap<QString, QVariant> changed;
    if (ok) {
        const QMap<QString, QVariant> before = Volume__getDbValues(volume);
        QDateTime when = QDateTime::fromString(p.last_read, Qt::ISODate);
        if (!when.isValid()) {
            // Keep DateLastRead as it is, as the CLI's --write does.
            when = QDateTime::fromString(before.value(QStringLiteral("DateLastRead")).toString(), Qt::ISODate);
        }
        if (!when.isValid()) {
            when = QDateTime::currentDateTimeUtc();
        }
        VolumeManager__updateBookmark(volume, serialized, before.value(QStringLiteral("adobe_location")).toInt(), when,
                                      0);
        Content__setPercentRead(volume, QVariant(p.percent));
        const QMap<QString, QVariant> after = Volume__getDbValues(volume);
        for (auto it = after.constBegin(); it != after.constEnd(); ++it) {
            if (it.value() != before.value(it.key())) {
                changed.insert(it.key(), it.value());
            }
        }
        log_line(QStringLiteral("write: changed %1").arg(row_diff(before, after)));
    } else {
        log_line(QStringLiteral("write: %1 is not in Nickel's database").arg(id));
    }
    Volume__dtor(volume);
    ok = ok && VolumeManager__updateFields(id, changed, {{QStringLiteral("ContentID"), id}}, db);
    ok = ok && drop_pages(db, p.stale_pages);
    const bool committed = DatabaseAdapter__endTransaction(db, ok);
    if (!ok || !committed) {
        log_line(QStringLiteral("write: rolled back (written %1, committed %2)").arg(ok).arg(committed));
        Repository__refreshCache_Volume(db);
        return false;
    }
    log_line(QStringLiteral("write: %1 through Nickel, %2 cached page(s) dropped").arg(position).arg(p.stale_pages.size()));
    return true;
}

// What a finished `sync --pending` means, for the log and the message.
static void pending_finished(QProcess *p)
{
    const QByteArray out = p->readAll();
    log_line(QStringLiteral("sync --pending (exit %1):\n%2").arg(p->exitCode()).arg(QString::fromUtf8(out).trimmed()));
    if (!machine_lines(out, "NO_INDEX").isEmpty()) {
        notice_no_index(out);
        return;
    }
    log_conflicts(out);
    int sent = 0, conflicts = 0;
    for (const QList<QByteArray> &f : machine_lines(out, "PENDING")) {
        if (f.value(1) == "sent") {
            ++sent;
        } else if (f.value(1) == "conflict") {
            ++conflicts;
        }
    }
    if (sent > 0) {
        toast(false, QStringLiteral("Vorgemerktes gesendet"),
              sent == 1 ? QStringLiteral("1 Buch") : QStringLiteral("%1 Bücher").arg(sent));
    }
    if (conflicts > 0) {
        toast(true, QStringLiteral("Vorgemerkt: Konflikt"), QStringLiteral("Server und Gerät bewegt, bleibt liegen"));
    }
}

// Runs a CLI command in the background and returns at once; `done` gets
// it when it finished (also after kPushWaitMs, when it is ended).
template <typename F>
static void start_cli(const QString &what, const QStringList &args, F done)
{
    QProcess *p = new QProcess(qApp);
    p->setProcessChannelMode(QProcess::MergedChannels);
    // Both signals named so they build with Qt 5 too (finished is
    // overloaded there, errorOccurred came in 5.6).
    QObject::connect(p, static_cast<void (QProcess::*)(int, QProcess::ExitStatus)>(&QProcess::finished), p,
                     [p, done](int, QProcess::ExitStatus) {
                         done(p);
                         p->deleteLater();
                     });
#if QT_VERSION >= QT_VERSION_CHECK(5, 6, 0)
    const auto failed = &QProcess::errorOccurred;
#else
    const auto failed = static_cast<void (QProcess::*)(QProcess::ProcessError)>(&QProcess::error);
#endif
    QObject::connect(p, failed, p, [p, what](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart) {
            log_line(QStringLiteral("%1: could not start the CLI: %2").arg(what, p->errorString()));
            toast(true, QStringLiteral("NickelInkSync startet nicht"), QStringLiteral("Siehe NickelInkSync.log"));
            p->deleteLater();
        }
    });
    // A run that hangs is ended; finished() then reports it.
    QTimer::singleShot(kPushWaitMs, p, [p, what] {
        if (p->state() != QProcess::NotRunning) {
            log_line(QStringLiteral("%1: no answer within %2 s, ended").arg(what).arg(kPushWaitMs / 1000));
            p->kill();
        }
    });
    p->start(QString::fromLatin1(kCli), args);
}

// One `sync --pending` at a time.
static bool g_pending_running = false;

// Sends the pending list in the background, if state.json has one; called
// whenever the server answered.
static void sync_pending()
{
    if (!g_active || kobalt_blocks() || g_pending_running || !has_pending()) {
        return;
    }
    log_line(QStringLiteral("sync --pending"));
    g_pending_running = true;
    QStringList args = book_args("sync", QString());
    args << QStringLiteral("--pending") << QStringLiteral("--emit") << QStringLiteral("--timeout")
         << QString::fromLatin1(kPushTimeout);
    start_cli(QStringLiteral("sync --pending"), args, [](QProcess *p) {
        g_pending_running = false;
        pending_finished(p);
    });
}

extern "C" __attribute__((visibility("default")))
void _nh_ReadingController__setVolume(ReadingController *self, const Volume *volume, const Bookmark *bookmark)
{
    if (!g_active) {
        ReadingController__setVolume(self, volume, bookmark);
        return;
    }
    if (kobalt_blocks()) {
        ReadingController__setVolume(self, volume, bookmark);
        notice_kobalt(kOpenToastDelayMs);
        return;
    }
    const QString id = Content__getId(volume, QString());
    g_open_id = id;
    g_not_synced.clear();
    g_unasked_id.clear();
    g_taken_id.clear();
    g_taken_page.clear();
    if (!id.startsWith(QString::fromLatin1(kSideloaded))) {
        ReadingController__setVolume(self, volume, bookmark);
        return;
    }
    g_open_row = row_of(volume);
    if (Content__getDbValues) {
        log_line(QStringLiteral("row at opening: %1").arg(row_text(g_open_row)));
    }
    if (!Bookmark__isNull(bookmark)) {
        log_line(QStringLiteral("open %1 with a bookmark (link, search): not pulled").arg(id));
        ReadingController__setVolume(self, volume, bookmark);
        return;
    }
    // isInternetAccessible() only reports Nickel's own connectivity check:
    // it stayed false for 10 s with Wi-Fi connected (device test). So it
    // only decides how long to wait; the CLI always tries, and fails fast
    // without a network.
    const bool wifi = wait_online(kOpenWifiMs);
    if (!wifi) {
        log_line(QStringLiteral("Nickel reports no internet; trying anyway"));
    }
    const Pulled p = pull(id);
    if (!p.ok) {
        // Pushed on leaving only if the position moved; without a network
        // the CLI then keeps it pending.
        g_unasked_id = id;
        g_unasked_at.clear();
        log_line(QStringLiteral("open %1 unchanged: the server could not be asked").arg(id));
        ReadingController__setVolume(self, volume, bookmark);
        QTimer::singleShot(kCheckOpenMs, qApp, [id] { note_opened_page(id, kCheckOpenTries); });
        // Without Wi-Fi the CLI only sees a timeout or no route; Nickel's
        // check is not reliable either way, so the text names both.
        const bool no_wifi = !wifi && (p.error_kind == "timeout" || p.error_kind == "network");
        toast(true, QStringLiteral("Laden fehlgeschlagen"),
              no_wifi ? QStringLiteral("Kein WLAN oder kein Server") : p.error, kOpenToastDelayMs);
        return;
    }
    // The server answered: send what is pending once the book is up.
    QTimer::singleShot(kPendingDelayMs, qApp, [] { sync_pending(); });
    if (p.conflict) {
        g_not_synced = id;
        g_not_synced_why = QStringLiteral("Konflikt, bleibt liegen");
        log_line(QStringLiteral("open %1 unchanged: a conflict, left alone; leaving it will not push").arg(id));
        ReadingController__setVolume(self, volume, bookmark);
        toast(true, QStringLiteral("Konflikt, nicht geladen"), QStringLiteral("Server und Gerät bewegt, bleibt liegen"),
              kOpenToastDelayMs);
        return;
    }
    if (p.unanswered) {
        g_not_synced = id;
        g_not_synced_why = kUnansweredReason;
        log_line(QStringLiteral("open %1 unchanged: the position needs an answer (confirm); leaving it will not push").arg(id));
        ReadingController__setVolume(self, volume, bookmark);
        toast(true, QStringLiteral("Nicht übernommen"), kUnansweredReason, kOpenToastDelayMs);
        return;
    }
    if (!p.not_taken.isEmpty()) {
        g_not_synced = id;
        g_not_synced_why = skip_reason(p.not_taken);
        log_line(QStringLiteral("open %1 unchanged: the server's position cannot be taken; leaving it will not push").arg(id));
        ReadingController__setVolume(self, volume, bookmark);
        toast(true, QStringLiteral("Nicht übernommen"), skip_reason(p.not_taken), kOpenToastDelayMs);
        return;
    }
    if (!p.found) {
        // The server holds nothing newer: the device is in line with it.
        ReadingController__setVolume(self, volume, bookmark);
        toast(false, QStringLiteral("Fortschritt ist aktuell"),
              QStringLiteral("Server hat nichts Neueres"), kOpenToastDelayMs);
        return;
    }
    log_line(QStringLiteral("open %1 at %2 (%3 %)").arg(id, QString::fromUtf8(p.position)).arg(p.percent));
    // Not in line with the server until check_open() finds Nickel at the
    // pulled place.
    g_not_synced = id;
    g_not_synced_why = QStringLiteral("Beim Öffnen nicht abgeglichen");
    if (g_kobo) {
        if (!write_through_nickel(id, p)) {
            g_not_synced = id;
            g_not_synced_why = QStringLiteral("Beim Öffnen nicht abgeglichen");
            ReadingController__setVolume(self, volume, bookmark);
            toast(true, QStringLiteral("Nicht übernommen"), QStringLiteral("Nickel schreibt nicht, siehe Log"),
                  kOpenToastDelayMs);
            return;
        }
        if (p.base.isEmpty() || !set_base(id, p.base)) {
            log_line(QStringLiteral("no base kept for %1: the next sync decides without it").arg(id));
        }
        // Nickel opens from the Volume it is handed; a fresh one from its
        // cache holds the written row.
        alignas(8) unsigned char fresh[kVolumeBytes];
        VolumeManager__getById(fresh, id, Device__getDbName(Device__getCurrentDevice()));
        g_open_row = row_of(fresh);
        ReadingController__setVolume(self, fresh, bookmark);
        Volume__dtor(fresh);
    } else {
        alignas(8) unsigned char copy[kVolumeBytes];
        Volume__copy(copy, volume);
        Content__setChapterIdBookmarked(copy, QVariant(p.position));
        Content__setPercentRead(copy, QVariant(p.percent));
        // The Volume Nickel passes in gets the position too: with only the
        // copy changed, opening within the chapter Nickel last saved went
        // to the chapter start; with both, Nickel opens at the point
        // (device tests). The setter detaches Nickel's Volume from data it
        // shares (the second word, logged), so nothing else in Nickel
        // changes.
        void *const shared_before = static_cast<void *const *>(volume)[1];
        Volume *original = const_cast<Volume *>(volume);
        Content__setChapterIdBookmarked(original, QVariant(p.position));
        Content__setPercentRead(original, QVariant(p.percent));
        g_open_row = row_of(copy);
        log_line(QStringLiteral("volume data: Nickel's %1 -> %2, copy %3")
                     .arg(reinterpret_cast<quintptr>(shared_before), 0, 16)
                     .arg(reinterpret_cast<quintptr>(static_cast<void *const *>(volume)[1]), 0, 16)
                     .arg(reinterpret_cast<quintptr>(reinterpret_cast<void *const *>(copy)[1]), 0, 16));
        ReadingController__setVolume(self, copy, bookmark);
        Volume__dtor(copy);
    }
    const QString target = QString::fromUtf8(p.position);
    const QString taken = p.from.isEmpty() ? QStringLiteral("%1 %").arg(p.percent)
                                           : QStringLiteral("%1 %, von %2").arg(p.percent).arg(p.from);
    QTimer::singleShot(kCheckOpenMs, qApp, [id, target, taken] { check_open(id, target, taken, kCheckOpenTries); });
}

// What a finished push means, for the log and the message; once the
// server answered, the pending list follows.
static void push_finished(const QString &id, QProcess *p)
{
    const QByteArray out = p->readAll();
    log_line(QStringLiteral("push %1 (exit %2):\n%3").arg(id).arg(p->exitCode()).arg(QString::fromUtf8(out).trimmed()));
    const QString not_sent = QStringLiteral("Nicht gesendet");
    if (!machine_lines(out, "NO_INDEX").isEmpty()) {
        notice_no_index(out);
        return;
    }
    if (!machine_lines(out, "PENDING").isEmpty()) {
        const QList<QList<QByteArray>> errors = machine_lines(out, "ERROR");
        const QByteArray kind = errors.isEmpty() ? QByteArray() : errors[0].value(0);
        toast(true, QStringLiteral("Fortschritt vorgemerkt"),
              kind == "server" ? QStringLiteral("Serverfehler, wird nachgeholt")
                               : QStringLiteral("Kein WLAN/Server, wird nachgeholt"));
        return;
    }
    if (p->exitStatus() != QProcess::NormalExit || p->exitCode() != 0) {
        toast(true, QStringLiteral("Senden fehlgeschlagen"), error_reason(out));
        return;
    }
    log_conflicts(out);
    const QList<QList<QByteArray>> sent = machine_lines(out, "SENT");
    const QList<QList<QByteArray>> skipped = machine_lines(out, "NOT_SENT");
    if (!sent.isEmpty()) {
        toast(false, QStringLiteral("Fortschritt gesendet"),
              QStringLiteral("%1 %").arg(QString::fromUtf8(sent[0].value(0))));
    } else if (!skipped.isEmpty()) {
        toast(true, not_sent, skip_reason(skipped[0].value(0)));
    } else if (!machine_lines(out, "CONFLICT").isEmpty()) {
        toast(true, not_sent, QStringLiteral("Konflikt: Server und Gerät bewegt"));
    } else if (!machine_lines(out, "UNANSWERED").isEmpty()) {
        toast(true, not_sent, kUnansweredReason);
    } else if (!machine_lines(out, "IN_SYNC").isEmpty()) {
        toast(false, QStringLiteral("Fortschritt ist aktuell"), QStringLiteral("Server hat schon diesen Stand"));
    }
    sync_pending();
}

// Starts the push of one book and returns at once; its output goes to the
// log when it finishes, and the outcome to a toast.
static void start_push(const QString &id)
{
    log_line(QStringLiteral("push %1").arg(id));
    QStringList args = book_args("push", id);
    args << QStringLiteral("--emit") << QStringLiteral("--timeout") << QString::fromLatin1(kPushTimeout);
    start_cli(QStringLiteral("push"), args, [id](QProcess *p) { push_finished(id, p); });
}

// What Nickel changed in the book's row while it was open, as its Volume
// cache holds it after saving (VolumeManager::updateBookmark keeps the
// cache in step with the database).
static void log_saved_row(const QString &id)
{
    if (!VolumeManager__getById || !Volume__isValid || !Content__getDbValues || g_open_row.isEmpty()) {
        return;
    }
    alignas(8) unsigned char saved[kVolumeBytes];
    VolumeManager__getById(saved, id, QString());
    if (Volume__isValid(saved)) {
        log_line(QStringLiteral("row on leaving, changed since opening: %1").arg(row_diff(g_open_row, row_of(saved))));
    }
    Volume__dtor(saved);
}

extern "C" __attribute__((visibility("default")))
void _nh_ReadingView__leavingReadingExperience(ReadingView *self)
{
    if (!g_active || kobalt_blocks()) {
        ReadingView__leavingReadingExperience(self);
        return;
    }
    // Still on the page taken from the server, or the one a book opened
    // without the server opened on? Asked before Nickel leaves.
    const QString leaving_at = current_location(self);
    const bool on_taken_page = !g_taken_page.isEmpty() && g_taken_id == g_open_id && leaving_at == g_taken_page;
    const bool on_unasked_page = !g_unasked_at.isEmpty() && g_unasked_id == g_open_id && leaving_at == g_unasked_at;
    // First Nickel saves the position.
    ReadingView__leavingReadingExperience(self);
    if (!g_open_id.startsWith(QString::fromLatin1(kSideloaded))) {
        return;
    }
    const QString id = g_open_id;
    log_saved_row(id);
    if (on_taken_page) {
        log_line(QStringLiteral("left %1 on the page taken from the server (%2): nothing to push").arg(id, g_taken_page));
        toast(false, QStringLiteral("Fortschritt ist aktuell"), QStringLiteral("Noch auf der Seite vom Server"));
        if (online()) {
            sync_pending();
        }
        return;
    }
    if (g_not_synced == id) {
        log_line(QStringLiteral("left %1, which is not in line with the server: not pushed").arg(id));
        toast(true, QStringLiteral("Nicht gesendet"), g_not_synced_why);
        if (online()) {
            sync_pending();
        }
        return;
    }
    if (on_unasked_page) {
        log_line(QStringLiteral("left %1 on the page it opened on without the server: nothing to push").arg(id));
        if (online()) {
            sync_pending();
        }
        return;
    }
    when_online(kWifiLeaveWaitS, [id] { start_push(id); });
}

static int nickelinksync_init()
{
    g_kobo = !Device__isTolino(Device__getCurrentDevice());
    if (!QFileInfo(QString::fromLatin1(kCli)).isExecutable()) {
        log_line(QStringLiteral("inactive: %1 is missing or not executable").arg(QString::fromLatin1(kCli)));
        return 0;
    }
    if (!QFileInfo::exists(QString::fromLatin1(kConfig))) {
        log_line(QStringLiteral("inactive: %1 is missing").arg(QString::fromLatin1(kConfig)));
        return 0;
    }
    g_notify = read_notify();
    if (kobalt_blocks()) {
        // Stays loaded, only to tell once that it is off. Not on opening a
        // book alone: with Kobalt as the default reader Nickel's own reader,
        // and so setVolume, is never called (device test). Nickel loads
        // plugins with its application running; the toast waits until its
        // main window is up.
        g_active = true;
        if (QCoreApplication::instance()) {
            notice_kobalt(kKobaltToastDelayMs);
        }
        return 0;
    }
    if (!kobalt_installed()) {
        QFile::remove(QString::fromLatin1(kKobaltNoticed));
    }
    if (g_kobo && !can_write_through_nickel()) {
        log_line(QStringLiteral("inactive: Nickel's write path is missing on this Kobo (docs/nickel-6.0-symbols.md)"));
        return 0;
    }
    // The messages are German; the locale is logged to learn whether the
    // device language can be told from it.
    log_line(QStringLiteral("active on a %1, CLI and configuration in %2; notify = %3, toasts %4, locale %5 (system %6)")
                 .arg(QLatin1String(g_kobo ? "Kobo" : "Tolino"))
                 .arg(QString::fromLatin1(kDir))
                 .arg(QLatin1String(g_notify == Notify::All ? "all" : g_notify == Notify::Errors ? "errors" : "none"))
                 .arg(QLatin1String(MainWindowController__toast ? "available" : "missing"))
                 .arg(QLocale().name(), QLocale::system().name()));
    g_active = true;
    return 0;
}

static struct nh_info NickelInkSync = {
    .name = "NickelInkSync",
    .desc = "Sync the reading position of sideloaded books with a KOSync server (Kobo 6.0, Tolino 5.18+)",
    .uninstall_flag = "/mnt/onboard/.adds/NickelInkSync/uninstall",
};

static struct nh_hook NickelInkSyncHook[] = {
    {
        .sym     = "_ZN17ReadingController9setVolumeERK6VolumeRK8Bookmark",
        .sym_new = "_nh_ReadingController__setVolume",
        .lib     = "libnickel.so.1.0.0",
        .out     = nh_symoutptr(ReadingController__setVolume),
        .desc    = "Pull the position when a book is opened",
    },
    {
        .sym     = "_ZN11ReadingView24leavingReadingExperienceEv",
        .sym_new = "_nh_ReadingView__leavingReadingExperience",
        .lib     = "libnickel.so.1.0.0",
        .out     = nh_symoutptr(ReadingView__leavingReadingExperience),
        .desc    = "Push the position when a book is left",
    },
    {0},
};

static struct nh_dlsym NickelInkSyncDlsym[] = {
    {.name = "_ZN6Device16getCurrentDeviceEv", .out = nh_symoutptr(Device__getCurrentDevice), .desc = "Kobo or Tolino"},
    {.name = "_ZNK6Device8isTolinoEv", .out = nh_symoutptr(Device__isTolino), .desc = "Kobo or Tolino"},
    {.name = "_ZNK7Content5getIdERK7QString", .out = nh_symoutptr(Content__getId), .desc = "Book ID"},
    {.name = "_ZNK8Bookmark6isNullEv", .out = nh_symoutptr(Bookmark__isNull), .desc = "Opened without a bookmark"},
    {.name = "_ZN6VolumeC1ERKS_", .out = nh_symoutptr(Volume__copy), .desc = "Copy of the book to open"},
    {.name = "_ZN6VolumeD1Ev", .out = nh_symoutptr(Volume__dtor), .desc = "Copy of the book to open"},
    {.name = "_ZN7Content22setChapterIdBookmarkedERK8QVariant", .out = nh_symoutptr(Content__setChapterIdBookmarked), .desc = "Pulled position"},
    {.name = "_ZN7Content14setPercentReadERK8QVariant", .out = nh_symoutptr(Content__setPercentRead), .desc = "Pulled percentage"},
    {.name = "_ZN20MainWindowController14sharedInstanceEv", .out = nh_symoutptr(MainWindowController__sharedInstance), .desc = "Reading view"},
    {.name = "_ZNK20MainWindowController11currentViewEv", .out = nh_symoutptr(MainWindowController__currentView), .desc = "Reading view"},
    {.name = "_ZN11ReadingView26bookmarkForCurrentLocationEv", .out = nh_symoutptr(ReadingView__bookmarkForCurrentLocation), .desc = "Current position"},
    {.name = "_ZNK8Bookmark16getStartLocationEv", .out = nh_symoutptr(Bookmark__getStartLocation), .desc = "Current position"},
    {.name = "_ZNK15ContentLocation8toStringEv", .out = nh_symoutptr(ContentLocation__toString), .desc = "Current position"},
    {.name = "_ZN8BookmarkD1Ev", .out = nh_symoutptr(Bookmark__dtor), .desc = "Current position"},
    {.name = "_ZN23WirelessWorkflowManager14sharedInstanceEv", .out = nh_symoutptr(WirelessWorkflowManager__sharedInstance), .desc = "Wi-Fi", .optional = true},
    {.name = "_ZN23WirelessWorkflowManager20isInternetAccessibleEv", .out = nh_symoutptr(WirelessWorkflowManager__isInternetAccessible), .desc = "Wi-Fi", .optional = true},
    {.name = "_ZN23WirelessWorkflowManager15connectWirelessEbb", .out = nh_symoutptr(WirelessWorkflowManager__connectWireless), .desc = "Wi-Fi", .optional = true},
    {.name = "_ZN20MainWindowController5toastERK7QStringS2_i", .out = nh_symoutptr(MainWindowController__toast), .desc = "Messages", .optional = true},
    {.name = "_ZN13VolumeManager7getByIdERK7QStringS2_", .out = nh_symoutptr(VolumeManager__getById), .desc = "Log", .optional = true},
    {.name = "_ZNK6Volume7isValidEv", .out = nh_symoutptr(Volume__isValid), .desc = "Log", .optional = true},
    {.name = "_ZNK7Content11getDbValuesEv", .out = nh_symoutptr(Content__getDbValues), .desc = "Log", .optional = true},
    {.name = "_ZNK6Device9getDbNameEv", .out = nh_symoutptr(Device__getDbName), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN15DatabaseAdapter16beginTransactionE7QString", .out = nh_symoutptr(DatabaseAdapter__beginTransaction), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN15DatabaseAdapter14endTransactionE7QStringb", .out = nh_symoutptr(DatabaseAdapter__endTransaction), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN15DatabaseAdapter12dbConnectionERK7QStringb", .out = nh_symoutptr(DatabaseAdapter__dbConnection), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZNK6Volume11getDbValuesEv", .out = nh_symoutptr(Volume__getDbValues), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN15ContentLocation6createE7QStringS0_", .out = nh_symoutptr(ContentLocation__create), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZNK15ContentLocation7isValidEv", .out = nh_symoutptr(ContentLocation__isValid), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN15ContentLocationD1Ev", .out = nh_symoutptr(ContentLocation__dtor), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN13VolumeManager14updateBookmarkER6VolumeRK7QStringiRK9QDateTimei", .out = nh_symoutptr(VolumeManager__updateBookmark), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN13VolumeManager12updateFieldsERK7QStringRK4QMapIS0_8QVariantES7_S2_", .out = nh_symoutptr(VolumeManager__updateFields), .desc = "Write (Kobo)", .optional = true},
    {.name = "_ZN10Repository12refreshCacheI6VolumeEEP5CacheIT_ERK7QString", .out = nh_symoutptr(Repository__refreshCache_Volume), .desc = "Write (Kobo)", .optional = true},
    {0},
};

NickelHook(
    .init  = nickelinksync_init,
    .info  = &NickelInkSync,
    .hook  = NickelInkSyncHook,
    .dlsym = NickelInkSyncDlsym,
)
