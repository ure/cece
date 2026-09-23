import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Ui

// Bar toggle for cece, the Burmilla cat that chases the cursor. The cat is a
// separate process (the Rust `cece` binary drawing on a click-through overlay
// layer). Left click mutes or unmutes the cat, right click makes it stay put
// or follow, middle click lets it out or sends it away. Both the sound and
// whether the cat is out are remembered across shell restarts.
BarWidget {
  id: root
  moduleName: "ure.cece"

  property bool running: false
  property bool staying: false
  property bool checked: false

  readonly property string command: String(setting("command", "cece"))
  readonly property bool enabled: setting("enabled", true) === true
  readonly property real speed: Number(setting("speed", 2)) || 2
  readonly property real scale: Number(setting("scale", 2)) || 2
  readonly property string skin: String(setting("skin", "burmilla"))
  readonly property bool quiet: setting("quiet", false) === true

  function argv() {
    var args = [root.command, "--speed", String(root.speed), "--scale", String(root.scale),
                "--skin", root.skin]
    if (root.quiet) args.push("--quiet")
    return args
  }

  // Detached, so the cat outlives a shell reload; cece itself refuses to run
  // twice, so every bar (one per monitor) may safely ask for it.
  function start() {
    Quickshell.execDetached(["bash", "-lc", 'exec "$@"', "bash"].concat(root.argv()))
    root.staying = false
    poll.restart()
  }

  function stop() {
    Quickshell.execDetached(["pkill", "-x", "cece"])
    root.running = false
    root.staying = false
  }

  function persist(values) {
    var shell = root.bar ? root.bar.shell : null
    if (!shell || typeof shell.updateEntryInline !== "function") return
    var next = { id: root.moduleName }
    for (var k in root.settings) if (k !== "id") next[k] = root.settings[k]
    for (var key in values) next[key] = values[key]
    shell.updateEntryInline(root.moduleName, next)
  }

  function toggle() {
    if (root.running) {
      root.stop()
      root.persist({ enabled: false })
    } else {
      root.start()
      root.persist({ enabled: true })
    }
  }

  // The running cat mutes on SIGUSR2, so it keeps its place; the setting makes
  // the next start match.
  function toggleSound() {
    if (root.running) Quickshell.execDetached(["pkill", "-USR2", "-x", "cece"])
    root.persist({ quiet: !root.quiet })
  }

  function toggleStay() {
    if (!root.running) return
    Quickshell.execDetached(["pkill", "-USR1", "-x", "cece"])
    root.staying = !root.staying
  }

  implicitWidth: button.implicitWidth
  implicitHeight: button.implicitHeight

  Process {
    id: probe
    command: ["pgrep", "-x", "cece"]
    onExited: function(code) {
      var was = root.running
      root.running = code === 0
      if (!root.running) root.staying = false
      // First look after the shell starts: bring the cat back if it was on.
      if (!root.checked) {
        root.checked = true
        if (!root.running && root.enabled) root.start()
      } else if (was && !root.running) {
        root.staying = false
      }
    }
  }

  Timer {
    id: poll
    interval: 3000
    repeat: true
    running: true
    triggeredOnStart: true
    onTriggered: probe.running = true
  }

  BarIconButton {
    id: button
    anchors.fill: parent
    bar: root.bar
    // A cat when out, a crossed-out paw when away; greyed while muted.
    text: root.running ? "󰄛" : "󰙗"
    dimmed: !root.running || root.quiet
    active: root.staying
    slotSize: Style.bar.iconSlot
    tooltipText: (!root.running
        ? "Cece is napping elsewhere"
        : root.staying ? "Cece is staying put" : "Cece is chasing your cursor")
      + (root.quiet ? " (muted)" : "")
      + "\nClick: " + (root.quiet ? "sound on" : "mute")
      + " · Right-click: " + (root.staying ? "follow" : "stay")
      + " · Middle-click: " + (root.running ? "send away" : "let out")
    onPressed: function(b) {
      if (b === Qt.RightButton) root.toggleStay()
      else if (b === Qt.MiddleButton) root.toggle()
      else root.toggleSound()
    }
  }
}
