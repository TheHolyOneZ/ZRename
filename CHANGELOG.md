# Changelog

All notable changes to ZRename are listed here, newest first.
Downloads for every version are on the
[releases page](https://github.com/TheHolyOneZ/ZRename/releases) and at
[zsync.eu/zrename](https://zsync.eu/zrename/).

## [0.1.3] — 2026-10-27

### New

- **Name your rules.** Any rule in the Rules panel can now be given its own
  name, such as *Remove website names* or *Replace dots with spaces*, so a stack
  of several Find & Replace rules is easy to read at a glance. Double-click a
  rule, press <kbd>F2</kbd>, or choose **Rename** from the rule's menu. The rule
  type stays visible in a smaller label beside the name and in the tooltip.
  Clear the name to go back to the rule type. Names are saved with presets
  (as `label` in the TOML) and kept when you restart the app.
  ([#1](https://github.com/TheHolyOneZ/ZRename/issues/1))

## [0.1.2] — 2026-10-01

### New

- **Leave alone.** Find & replace, Case, Remove, Trim and Sanitise rules can be
  told which parts of a name to keep exactly as they are, with `*` as a
  wildcard. Leave-alone patterns are saved with presets.
- **Clear the history.** The History panel has a **Clear** button.
- **Close the folder.** A **✕** next to the folder name, or *Close the folder*
  in the command palette, empties the file list.

### Fixed

- Reading a large folder, for example on a network share, no longer makes the
  window stop responding. It runs in the background with a progress box.
- Undo no longer appears where the Apply button was, so it cannot be hit by
  accident straight after applying.
- The folder dialog opens in the current folder instead of the home directory.

## [0.1.1] — 2026-09-23

### Fixed

- Drop-down menus no longer close while you scroll them.
- The window can be resized from all four edges and corners again, including
  on Wayland.

### Improved

- <kbd>↑</kbd>, <kbd>↓</kbd> and <kbd>Enter</kbd> move through and pick items
  in open menus.

## [0.1.0] — 2026-09-04

- First public release.

[0.1.3]: https://github.com/TheHolyOneZ/ZRename/releases/tag/0.1.3
[0.1.2]: https://github.com/TheHolyOneZ/ZRename/releases/tag/0.1.2
[0.1.1]: https://github.com/TheHolyOneZ/ZRename/releases/tag/0.1.1
[0.1.0]: https://github.com/TheHolyOneZ/ZRename/releases/tag/1.0.0
