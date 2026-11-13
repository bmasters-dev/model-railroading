#!/bin/sh
# Tries this addon as just built on the bench, before it is released: runs an example app (or any
# Leah script) with Leah's development build (every official addon, windows too) and this addon,
# on your own screen. Your roster is the one the installed IDE keeps, so the locomotives you saved
# there are here too. (The owner, 2026-11-11: no waiting for releases to try a change.)
#
#     sh ~/Model_Railroading/leah-addon/tools/bench.sh phone_server        an example, by its name
#     sh ~/Model_Railroading/leah-addon/tools/bench.sh ~/somewhere/my.leah  any script
#
# It builds the addon first when its code changed (a few seconds); Leah's development build is
# Claude's to keep up to date (~/Leah_Dev/target/all_addons/debug/leah).
set -eu
here="$(cd "$(dirname "$0")/.." && pwd)"
leah="$HOME/Leah_Dev/target/all_addons/debug/leah"
app_folder="$HOME/programs/Leah_IDE/leah-ide_data"
if [ $# -lt 1 ]; then
    echo "bench: give an example's name (phone_server, dcc_throttle, decoder_workshop, ...) or a script's path" >&2
    ls "$here/examples" | sed -n 's/\.leah$//p' | sed 's/^/    /' >&2
    exit 2
fi
script="$1"
shift
case "$script" in
    */*|*.leah) ;;
    *) script="$here/examples/$script.leah" ;;
esac
if [ ! -f "$script" ]; then
    echo "bench: there is no $script" >&2
    exit 2
fi
if [ ! -x "$leah" ]; then
    echo "bench: Leah's development build isn't there ($leah): ask Claude to build it (tools/build_all_addons.sh)" >&2
    exit 1
fi
(cd "$here" && cargo build --quiet)
addons="$HOME/Leah_Temp/bench/3rd-party-addons"
mkdir -p "$addons"
cp "$here/target/debug/libleah_addon_model_railroading.so" "$addons/model_railroading.so"
# (Its help from the pages, as a release carries it: hover help in an IDE run from here.)
python3 -I "$here/tools/docs.py" help --to "$addons/model_railroading.help.json" 2>/dev/null || true
echo "bench: $(basename "$script") with $("$leah" --version | head -1) and the addon as built" >&2
cd "$(dirname "$script")"
# (LEAH_PHONE_LOG: what each phone and the phone server say, on this terminal.)
exec env LEAH_3RD_PARTY_ADDONS="$addons" LEAH_USER_ADDONS="" LEAH_APP_FOLDER="$app_folder" LEAH_PHONE_LOG=1 "$leah" "$script" "$@"
