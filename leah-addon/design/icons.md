# The addon's icons

`model_railroading.icons` (beside `addon.toml`) holds the icons the Leah IDE shows for `dcc_base`,
`locomotive`, and `dcc_program_job` in its library and tray, at 16 and 32 pixels: a zip with `<kind>-16.png`,
`<kind>-32.png`, and `index.json`, as Leah's addon packs are. The first two were drawn by the IDE's
`tools/make_icons.leah` (bmasters-dev/leah-ide) while the addon was one of Leah's own. Since 2026-11-10
the addon draws its own with `tools/make_icons.leah` (a Leah with the graphics and compression
addons: `leah tools/make_icons.leah [preview.png]`), on a copy of the IDE tool's helpers
(`tools/icon_helpers.leah`), so they come out the same; the drawings live there. As they were in the
IDE's tool:

```
` -- model railroading --
` A base station: a box, its DCC square wave on its face, and two rails below.
$icons["dcc_base"] = function() {
    ic.rbox(0.5, 1.5, 15, 9, ic.face, ic.edge)
    ic.stroke([[2.5, 7.5], [4.5, 7.5], [4.5, 4], [6.5, 4], [6.5, 7.5], [9.5, 7.5], [9.5, 4], [11.5, 4], [11.5, 7.5], [13.5, 7.5]], ic.accent, 1.2)
    ic.fill(0, 12.5, 16, 1, ic.edge)
    ic.fill(0, 14.5, 16, 1, ic.edge)
}
` A locomotive: its boiler, cab, and wheels, on a rail.
$icons["locomotive"] = function() {
    ic.rfill(1, 6, 9, 5, ic.accent, 1)
    ic.rfill(9, 3, 6, 8, ic.accent_dark, 1)
    ic.fill(10.5, 4.5, 3, 2.5, ic.accent_light)
    ic.fill(3, 3.5, 2, 2.5, ic.ink)
    ic.circle(2, 10, 4, 4, ic.ink)
    ic.circle(6.5, 10, 4, 4, ic.ink)
    ic.circle(11, 10, 4, 4, ic.ink)
    ic.fill(0, 14.5, 16, 1, ic.edge)
}
```
