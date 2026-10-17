pragma Singleton
import QtQuick
import dev.skepy.wyre

// The connection list's rows, kept in step with the daemon's snapshots. Rows
// whose connection closed or whose process exited linger, faded, for a few
// seconds; rows hidden by a collapse or search go at once.
QtObject {
    id: rows

    // One { kind, listIndex, row } per row on screen. `row` is a group ("G"),
    // process ("P") or connection ("C") row from rows.rs, with `gone: true`
    // while fading out; `kind` is its type and `listIndex` its place in its
    // own list (see listIndexes).
    readonly property ListModel model: ListModel {}

    // The detail pane's connection, or null. Refreshed with every snapshot,
    // and kept after the connection is gone.
    property var selected: null
    readonly property string selectedKey: selected ? keyOf(selected) : ""

    // Emitted after every change to `model`.
    signal updated()

    readonly property int goneTime: 3000 // ms

    property var live: []        // the latest snapshot's rows
    property var shown: []       // what `model` holds: { key, json, listIndex, row } per row
    property var gone: new Map() // key → { row, until } of every row fading out
    property bool viewChanging: false

    property Connections snapshots: Connections {
        target: WyreModel
        function onSnapshot_jsonChanged() {
            rows.update(JSON.parse(WyreModel.snapshot_json))
        }
    }

    property Timer expiry: Timer {
        onTriggered: rows.update(rows.live)
    }

    // Runs `change`, a backend call that rebuilds the rows for a new view
    // (search, collapse, filter), so the rows it drops vanish at once rather
    // than fade as gone.
    function changeView(change) {
        viewChanging = true
        change()
        viewChanging = false
    }

    // A row's identity, the same from one snapshot to the next.
    function keyOf(row) {
        if (row.t === "G")
            return "G|" + row.name
        if (row.t === "P")
            return "P|" + row.pid
        return "C|" + row.pid + "|" + row.proto + "|" + row.local + "|" + row.remote
    }

    function update(snapshot) {
        const now = Date.now()
        const liveKeys = new Set(snapshot.map(keyOf))

        for (const key of liveKeys)
            gone.delete(key) // it's back

        // Without the daemon, the rows go at once rather than look closed.
        if (!viewChanging && WyreModel.connected) {
            let group = ""
            for (const entry of shown) {
                const row = entry.row
                if (row.t === "G")
                    group = row.name
                if (row.gone || liveKeys.has(entry.key) || gone.has(entry.key))
                    continue
                const goneRow = Object.assign({}, row, { gone: true })
                if (row.t === "P")
                    goneRow.group = group // to be put back under it
                gone.set(entry.key, { row: goneRow, until: now + goneTime })
            }
        }
        for (const [key, entry] of gone) {
            if (entry.until <= now)
                gone.delete(key)
        }

        live = snapshot
        show(withGoneRows(snapshot))

        // Without the daemon, refreshHidden() marks it gone, so the pane dims it
        // (as offline) until the daemon answers again.
        if (selected) {
            const fresh = snapshot.find(row => row.t === "C" && keyOf(row) === selectedKey)
            if (fresh)
                selected = fresh
            else if (gone.has(selectedKey))
                selected = gone.get(selectedKey).row
            else
                refreshHidden()
        }

        scheduleExpiry()
        updated()
    }

    // Refreshes the selected connection from the backend while its row is
    // hidden, and marks it gone once it has closed.
    function refreshHidden() {
        const json = WyreModel.connection_json(
            selected.pid, selected.proto, selected.local, selected.remote)
        if (json !== "")
            selected = JSON.parse(json)
        else if (!selected.gone)
            selected = Object.assign({}, selected, { gone: true })
    }

    // The live rows with the gone ones put back under their program or
    // process, if that's still listed and expanded.
    function withGoneRows(snapshot) {
        const result = snapshot.slice()
        for (const kind of ["G", "P", "C"]) { // parents before their children
            for (const { row } of gone.values()) {
                if (row.t !== kind)
                    continue
                const at = placeFor(result, row)
                if (at !== -1)
                    result.splice(at, 0, row)
            }
        }
        return result
    }

    // Where a gone row goes back: a program at the end, a process or
    // connection after its parent's last row. -1 if the parent is missing or
    // collapsed.
    function placeFor(list, goneRow) {
        if (goneRow.t === "G")
            return list.length

        const parent = goneRow.t === "P"
            ? list.findIndex(row => row.t === "G" && row.name === goneRow.group)
            : list.findIndex(row => row.t === "P" && row.pid === goneRow.pid)
        if (parent === -1 || !list[parent].exp)
            return -1

        let end = parent + 1
        while (end < list.length && depth(list[end]) > depth(list[parent]))
            end++
        return end
    }

    function depth(row) {
        return row.t === "G" ? 0 : row.t === "P" ? 1 : 2
    }

    // Brings `model` to `target`, matching rows by key, so a row keeps its
    // delegate and a delegate only hears of changes to its own row.
    function show(target) {
        const indexes = listIndexes(target)
        const keys = target.map(keyOf)
        const wanted = new Set(keys)

        // Rows that went are removed first, in runs, so the rows after them
        // don't each have to be moved up past them.
        for (let end = shown.length; end > 0;) {
            if (wanted.has(shown[end - 1].key)) {
                end--
                continue
            }
            let start = end - 1
            while (start > 0 && !wanted.has(shown[start - 1].key))
                start--
            model.remove(start, end - start)
            shown.splice(start, end - start)
            end = start
        }
        const present = new Set(shown.map(entry => entry.key))

        for (let i = 0; i < target.length; i++) {
            const row = target[i]
            const entry = {
                key: keys[i],
                json: JSON.stringify(row),
                listIndex: indexes[i],
                row: row,
            }
            // A new row needn't be looked for. One that's here is at i or after,
            // as those before are already in place.
            let at = shown.length
            if (present.has(entry.key)) {
                at = i
                while (at < shown.length && shown[at].key !== entry.key)
                    at++
            }

            if (at === shown.length) {
                model.insert(i, { kind: row.t, listIndex: entry.listIndex, row: row })
                shown.splice(i, 0, entry)
                continue
            }
            if (at !== i) {
                model.move(at, i, 1)
                shown.splice(i, 0, shown.splice(at, 1)[0])
            }
            if (shown[i].json !== entry.json || shown[i].listIndex !== entry.listIndex) {
                model.set(i, { kind: row.t, listIndex: entry.listIndex, row: row })
                shown[i] = entry
            }
        }
        if (shown.length > target.length) {
            model.remove(target.length, shown.length - target.length)
            shown.length = target.length
        }
    }

    // Each row's index among its siblings, for striping, so a row's color
    // only changes when its own list does.
    function listIndexes(list) {
        const indexes = []
        let group = 0
        let process = 0
        let connection = 0
        for (const row of list) {
            if (row.t === "G") {
                indexes.push(group++)
                process = 0
            } else if (row.t === "P") {
                indexes.push(process++)
                connection = 0
            } else {
                indexes.push(connection++)
            }
        }
        return indexes
    }

    // Wakes up just after the next gone row is due to be removed.
    function scheduleExpiry() {
        let next = Infinity
        for (const entry of gone.values())
            next = Math.min(next, entry.until)
        if (next === Infinity) {
            expiry.stop()
            return
        }
        expiry.interval = Math.max(16, next - Date.now() + 60)
        expiry.restart()
    }
}
