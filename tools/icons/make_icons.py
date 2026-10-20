# Writes data/icons/*.svg: the item, HUD, figure, portrait and map-mark icons.
# Protection of the Spheres' palette (data/look.json): plum ink outlines,
# two-tone facets lit from the upper left, a small paper highlight.
import pathlib, sys
OUT = pathlib.Path(sys.argv[1])
INK = "#2b2140"
def st(w="2.6", c=INK):
    return f'stroke="{c}" stroke-width="{w}" stroke-linejoin="round" stroke-linecap="round"'
S = st()
STEEL, STEEL_D, STEEL_L = "#e4e1ec", "#aea8c2", "#fbf9ff"
GOLD, GOLD_D = "#f2bf55", "#c98a2e"
WOOD, WOOD_D = "#b98252", "#86592f"
GRIP = "#6f4a35"
SKY, SKY_D, SKY_L = "#5f97dc", "#3f6db3", "#a9cbf2"
LAMP = "#ffd68c"
ACC, ACC_D = "#e07b4c", "#b65a33"
PAPER = "#fbf5e9"
ROCK, ROCK_D, ROCK_L = "#a99c8a", "#857868", "#c8bca9"
LEAF, LEAF_D = "#6aa65a", "#4b8443"
SKIN, SKIN_D = "#f4d9bd", "#e2b892"

def svg(body, view="0 0 64 64"):
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view}">{body}</svg>\n'

icons = {}

# swords: drawn on a diagonal, hilt at the lower left
def sword(length, width, guard):
    # blade from (14,50) toward (14+length, 50-length) as a kite polygon
    x0, y0 = 20, 44
    x1, y1 = x0 + length, y0 - length
    w = width
    blade_l = f"{x0-w},{y0-w} {x1-2},{y1-1} {x1},{y1} {x0},{y0}"
    blade_d = f"{x0+w},{y0+w} {x1+1},{y1+2} {x1},{y1} {x0},{y0}"
    g = guard
    return (
        f'<polygon points="{x0-w},{y0-w} {x1-2},{y1-1} {x1},{y1} {x1+1},{y1+2} {x0+w},{y0+w}" fill="{STEEL_D}" {S}/>'
        f'<polygon points="{blade_l}" fill="{STEEL}"/>'
        f'<polyline points="{x0+2},{y0-2} {x1-3},{y1+3}" stroke="{STEEL_L}" stroke-width="1.6" stroke-linecap="round" fill="none"/>'
        f'<polygon points="{x0-g},{y0-g+4} {x0-g+4},{y0-g} {x0+g},{y0+g-4} {x0+g-4},{y0+g}" fill="{GOLD}" {S}/>'
        f'<polygon points="{x0-1},{y0+3} {x0+3},{y0-1} {x0-7},{y0+11} {x0-11},{y0+7}" fill="{GRIP}" {S}/>'
        f'<circle cx="{x0-11}" cy="{y0+11}" r="3.6" fill="{GOLD}" {S}/>'
        f'<circle cx="{x0-12}" cy="{y0+10}" r="1.1" fill="{PAPER}"/>'
    )

icons["short_sword"] = svg(sword(22, 3.2, 8), view="3 13 46 46")
icons["sword"] = svg(sword(30, 4.2, 10) + f'<polyline points="22,40 46,16" stroke="{STEEL_D}" stroke-width="1.4" stroke-linecap="round" fill="none"/>', view="4 8 51 51")

icons["boomerang"] = svg(
    f'<path d="M10 40 C 16 18, 32 10, 52 14 L 54 22 C 38 21, 26 28, 20 46 Z" fill="{WOOD_D}" {S}/>'
    f'<path d="M10 40 C 16 18, 32 10, 52 14 L 52.6 17 C 34 15, 22 24, 15 42 Z" fill="{WOOD}"/>'
    f'<path d="M10 40 C 16 18, 32 10, 52 14 L 54 22 C 38 21, 26 28, 20 46 Z" fill="none" {S}/>'
    f'<path d="M24 22 L 30 19 L 33 25 L 27 28 Z" fill="{ACC}" {st("1.8")}/>'
    f'<path d="M14 32 L 19 29 L 22 35 L 17 38 Z" fill="{SKY}" {st("1.8")}/>'
    f'<circle cx="46" cy="16" r="1.4" fill="{PAPER}"/>'
)

icons["rock"] = svg(
    f'<polygon points="12,44 18,26 32,18 46,22 54,36 48,48 26,50" fill="{ROCK_D}" {S}/>'
    f'<polygon points="12,44 18,26 32,18 30,34" fill="{ROCK_L}"/>'
    f'<polygon points="32,18 46,22 54,36 30,34" fill="{ROCK}"/>'
    f'<polygon points="12,44 30,34 26,50" fill="{ROCK}"/>'
    f'<polygon points="12,44 18,26 32,18 46,22 54,36 48,48 26,50" fill="none" {S}/>'
    f'<polyline points="18,26 30,34 54,36" fill="none" stroke="{INK}" stroke-width="1.3" stroke-linejoin="round" opacity="0.45"/>'
    f'<polyline points="30,34 26,50" fill="none" stroke="{INK}" stroke-width="1.3" opacity="0.45"/>'
    f'<circle cx="24" cy="27" r="1.5" fill="{PAPER}"/>',
    view="7 8 50 50",
)

icons["knife"] = svg(
    f'<polygon points="26,38 48,12 50,14 30,42" fill="{STEEL_D}" {S}/>'
    f'<polygon points="26,38 48,12 49,13 28,40" fill="{STEEL}"/>'
    f'<polygon points="26,38 48,12 50,14 30,42" fill="none" {S}/>'
    f'<polygon points="22,36 28,34 32,40 26,44" fill="{GRIP}" {S}/>'
    f'<polygon points="20,40 24,44 18,52 14,48" fill="{GRIP}" {S}/>'
    f'<circle cx="13" cy="53" r="4" fill="none" {S}/>',
    view="5 7 50 50",
)

icons["bandage"] = svg(
    f'<path d="M36 40 C 44 46, 52 44, 56 52 L 48 56 C 44 50, 38 50, 30 46 Z" fill="{PAPER}" {S}/>'
    f'<ellipse cx="28" cy="32" rx="16" ry="16" fill="#e9dcc2" {S}/>'
    f'<ellipse cx="28" cy="32" rx="11" ry="11" fill="{PAPER}" {st("1.6")}/>'
    f'<ellipse cx="28" cy="32" rx="5" ry="5" fill="#e9dcc2" {st("1.6")}/>'
    f'<path d="M22 14 L 34 14 L 34 18 L 22 18 Z" fill="{ACC}" {st("1.6")}/>'
    f'<path d="M26 10 L 30 10 L 30 22 L 26 22 Z" fill="{ACC}" {st("1.6")}/>'
)

icons["chest_key"] = svg(
    f'<circle cx="20" cy="22" r="11" fill="{GOLD}" {S}/>'
    f'<circle cx="20" cy="22" r="4.6" fill="#f6e6c4" {st("2")}/>'
    f'<polygon points="27,29 50,52 46,56 23,33" fill="{GOLD_D}" {S}/>'
    f'<polygon points="40,46 46,40 50,44 44,50" fill="{GOLD}" {st("2")}/>'
    f'<polygon points="34,40 39,35 42,38 37,43" fill="{GOLD}" {st("2")}/>'
    f'<circle cx="15" cy="16" r="1.6" fill="{PAPER}"/>'
)

stripe = f'stroke="{LAMP}" stroke-width="3.2" stroke-linecap="round" fill="none"'
icons["hood"] = svg(
    f'<path d="M14 50 C 12 30, 20 12, 34 10 C 48 10, 54 26, 52 50 Z" fill="{SKY_D}" {S}/>'
    f'<path d="M14 50 C 12 30, 20 12, 34 10 C 30 20, 26 34, 26 50 Z" fill="{SKY}"/>'
    f'<path d="M14 50 C 12 30, 20 12, 34 10 C 48 10, 54 26, 52 50 Z" fill="none" {S}/>'
    f'<path d="M22 46 C 22 32, 28 24, 34 24 C 41 24, 45 32, 44 46 Z" fill="{SKIN_D}" {S}/>'
    f'<path d="M24 46 C 24 34, 29 27, 34 27" fill="none" stroke="{SKIN}" stroke-width="3" stroke-linecap="round"/>'
    f'<path d="M36 12 L 30 20 L 38 20 L 32 28" {stripe}/>'
)
icons["jacket"] = svg(
    f'<path d="M20 14 L 26 12 L 32 18 L 38 12 L 44 14 L 54 22 L 50 34 L 46 32 L 46 54 L 18 54 L 18 32 L 14 34 L 10 22 Z" fill="{SKY_D}" {S}/>'
    f'<path d="M20 14 L 26 12 L 32 18 L 32 54 L 18 54 L 18 32 L 14 34 L 10 22 Z" fill="{SKY}"/>'
    f'<path d="M20 14 L 26 12 L 32 18 L 38 12 L 44 14 L 54 22 L 50 34 L 46 32 L 46 54 L 18 54 L 18 32 L 14 34 L 10 22 Z" fill="none" {S}/>'
    f'<polyline points="32,18 32,54" stroke="{INK}" stroke-width="1.8" fill="none"/>'
    f'<path d="M22 26 L 28 34 L 22 34 L 28 42" {stripe}/>'
    f'<path d="M42 26 L 36 34 L 42 34 L 36 42" {stripe}/>'
)
icons["gloves"] = svg(
    f'<path d="M20 52 L 18 30 L 16 18 C 16 14, 21 14, 22 18 L 24 26 L 24 12 C 24 8, 29 8, 29 12 L 30 24 L 32 10 C 32 6, 37 6, 37 10 L 37 25 L 40 13 C 41 9, 46 10, 45 14 L 43 30 L 48 26 C 51 23, 55 27, 52 31 L 42 46 L 42 52 Z" fill="{SKY_D}" {S}/>'
    f'<path d="M20 52 L 18 30 L 16 18 C 16 14, 21 14, 22 18 L 24 26 L 24 12 C 24 8, 29 8, 29 12 L 30 24 L 30 52 Z" fill="{SKY}"/>'
    f'<path d="M20 52 L 18 30 L 16 18 C 16 14, 21 14, 22 18 L 24 26 L 24 12 C 24 8, 29 8, 29 12 L 30 24 L 32 10 C 32 6, 37 6, 37 10 L 37 25 L 40 13 C 41 9, 46 10, 45 14 L 43 30 L 48 26 C 51 23, 55 27, 52 31 L 42 46 L 42 52 Z" fill="none" {S}/>'
    f'<rect x="18" y="46" width="26" height="10" rx="2" fill="{LAMP}" {S}/>'
    f'<path d="M26 34 L 31 30 L 31 36 L 36 32" stroke="{LAMP}" stroke-width="2.6" stroke-linecap="round" fill="none"/>'
)
icons["trousers"] = svg(
    f'<path d="M18 10 L 46 10 L 50 54 L 37 54 L 32 26 L 27 54 L 14 54 Z" fill="{SKY_D}" {S}/>'
    f'<path d="M18 10 L 32 10 L 32 26 L 27 54 L 14 54 Z" fill="{SKY}"/>'
    f'<path d="M18 10 L 46 10 L 50 54 L 37 54 L 32 26 L 27 54 L 14 54 Z" fill="none" {S}/>'
    f'<rect x="17" y="9" width="30" height="6" rx="1.5" fill="{SKY_L}" {st("2")}/>'
    f'<path d="M21 22 L 18 50" {stripe}/>'
    f'<path d="M43 22 L 46 50" {stripe}/>'
)
icons["boots"] = svg(
    f'<path d="M20 10 L 36 10 L 36 36 L 52 42 C 56 44, 56 52, 52 52 L 16 52 L 16 30 Z" fill="{SKY_D}" {S}/>'
    f'<path d="M20 10 L 30 10 L 28 36 L 26 52 L 16 52 L 16 30 Z" fill="{SKY}"/>'
    f'<path d="M20 10 L 36 10 L 36 36 L 52 42 C 56 44, 56 52, 52 52 L 16 52 L 16 30 Z" fill="none" {S}/>'
    f'<path d="M14 52 L 56 52 L 56 56 L 14 56 Z" fill="{INK}"/>'
    f'<rect x="18" y="8" width="20" height="6" rx="1.5" fill="{SKY_L}" {st("2")}/>'
    f'<path d="M40 18 L 52 14 M 40 25 L 54 22 M 40 32 L 50 30" stroke="{LAMP}" stroke-width="2.8" stroke-linecap="round"/>'
    f'<path d="M24 22 L 30 28 L 24 30 L 30 38" stroke="{LAMP}" stroke-width="2.6" stroke-linecap="round" fill="none"/>'
)

# the imbue: an upright blade in a ring of coloured shapes (CW-10's orbs)
icons["imbue"] = svg(
    f'<circle cx="32" cy="32" r="22" fill="#efe6ff" opacity="0.55"/>'
    f'<polygon points="30,48 30,14 32,8 34,14 34,48" fill="{STEEL}" {S}/>'
    f'<polyline points="32,12 32,44" stroke="{STEEL_L}" stroke-width="1.4"/>'
    f'<rect x="23" y="46" width="18" height="4.5" rx="1.5" fill="{GOLD}" {st("2")}/>'
    f'<rect x="30" y="50" width="4" height="8" fill="{GRIP}" {st("2")}/>'
    f'<circle cx="14" cy="22" r="4" fill="#ff8fb4" {st("2")}/>'
    f'<polygon points="50,16 55,22 49,26" fill="{LAMP}" {st("2")}/>'
    f'<rect x="47" y="38" width="7" height="7" transform="rotate(20 50.5 41.5)" fill="#8be0c2" {st("2")}/>'
    f'<polygon points="12,40 16,36 20,40 16,44" fill="{SKY_L}" {st("2")}/>'
)

icons["coin"] = svg(
    f'<circle cx="32" cy="32" r="20" fill="{GOLD_D}" {S}/>'
    f'<path d="M32 12 A 20 20 0 0 0 14 42 L 32 32 Z" fill="{GOLD}"/>'
    f'<circle cx="32" cy="32" r="20" fill="none" {S}/>'
    f'<polygon points="32,20 42,26 42,38 32,44 22,38 22,26" fill="{GOLD}" {st("2")}/>'
    f'<polygon points="32,20 42,26 32,32 22,26" fill="#ffe29a"/>'
    f'<polygon points="32,20 42,26 42,38 32,44 22,38 22,26" fill="none" {st("2")}/>'
    f'<circle cx="24" cy="21" r="1.8" fill="{PAPER}"/>'
)

# the Equipment screen's figure: a quiet paper-doll silhouette, head a
# quarter of the height (CW-17), arms out so the slots can point at them
FIG, FIG_D = "#e9dcc2", "#d6c4a3"
icons["figure"] = svg(
    f'<g {st("3")}>'
    f'<path d="M72 200 L 84 300 L 100 300 L 104 220 L 112 300 L 128 300 L 140 200 Z" fill="{FIG_D}"/>'
    f'<path d="M66 108 C 66 96, 78 90, 106 90 C 134 90, 146 96, 146 108 L 150 204 L 62 204 Z" fill="{FIG}"/>'
    f'<path d="M66 110 L 40 170 L 50 178 L 76 128" fill="{FIG}"/>'
    f'<path d="M146 110 L 172 170 L 162 178 L 136 128" fill="{FIG}"/>'
    f'<circle cx="45" cy="182" r="9" fill="{FIG_D}"/>'
    f'<circle cx="167" cy="182" r="9" fill="{FIG_D}"/>'
    f'<path d="M78 300 L 104 300 L 104 314 L 72 314 Z" fill="{FIG_D}"/>'
    f'<path d="M108 300 L 134 300 L 140 314 L 108 314 Z" fill="{FIG_D}"/>'
    f'<circle cx="106" cy="54" r="38" fill="{FIG}"/>'
    f'</g>'
    f'<circle cx="94" cy="56" r="3.5" fill="{INK}"/><circle cx="118" cy="56" r="3.5" fill="{INK}"/>'
    f'<path d="M98 70 Q 106 76 114 70" fill="none" stroke="{INK}" stroke-width="2.6" stroke-linecap="round"/>'
    f'<circle cx="86" cy="66" r="5" fill="#f2b8a0" opacity="0.6"/><circle cx="126" cy="66" r="5" fill="#f2b8a0" opacity="0.6"/>',
    view="0 0 212 320",
)

# portraits for the dialogue box: Protection of the Spheres' round faces
def face(skin, hair, extra):
    return svg(
        f'<circle cx="40" cy="44" r="27" fill="{skin}" {S}/>'
        + extra.format(hair=hair, ink=INK) +
        f'<circle cx="31" cy="46" r="3.2" fill="{INK}"/><circle cx="49" cy="46" r="3.2" fill="{INK}"/>'
        f'<circle cx="30" cy="45" r="1" fill="{PAPER}"/><circle cx="48" cy="45" r="1" fill="{PAPER}"/>'
        f'<circle cx="25" cy="54" r="4" fill="#f2a58a" opacity="0.55"/><circle cx="55" cy="54" r="4" fill="#f2a58a" opacity="0.55"/>',
        view="0 0 80 80",
    )
icons["portrait_trainer"] = face(SKIN_D, "#d9d4cc",
    '<path d="M16 54 C 18 74, 62 74, 64 54 C 56 62, 24 62, 16 54 Z" fill="{hair}" stroke="{ink}" stroke-width="2.6" stroke-linejoin="round"/>'
    '<path d="M32 58 Q 40 62 48 58" fill="none" stroke="{ink}" stroke-width="2.4" stroke-linecap="round"/>'
    '<path d="M14 34 C 20 20, 60 20, 66 34 L 66 38 L 14 38 Z" fill="#e07b4c" stroke="{ink}" stroke-width="2.6" stroke-linejoin="round"/>'
    '<path d="M64 34 L 74 30 L 72 40 Z" fill="#e07b4c" stroke="{ink}" stroke-width="2.2" stroke-linejoin="round"/>')
icons["portrait_merchant"] = face(SKIN, "#7a5a3a",
    '<path d="M8 30 L 72 30 L 64 24 L 56 8 L 24 8 L 16 24 Z" fill="#b98252" stroke="{ink}" stroke-width="2.6" stroke-linejoin="round"/>'
    '<path d="M18 24 L 62 24" stroke="#e07b4c" stroke-width="4"/>'
    '<path d="M30 58 Q 40 66 50 58" fill="none" stroke="{ink}" stroke-width="2.4" stroke-linecap="round"/>'
    '<path d="M28 54 Q 40 50 52 54" fill="none" stroke="{hair}" stroke-width="4" stroke-linecap="round"/>')
# M5: the player, for the dream line: brown hair under a blue nightcap
icons["portrait_you"] = face(SKIN, "#6b4528",
    '<path d="M14 40 C 12 22, 66 22, 66 40 C 58 32, 22 32, 14 40 Z" fill="{hair}" stroke="{ink}" stroke-width="2.6" stroke-linejoin="round"/>'
    '<path d="M14 32 C 18 12, 52 6, 70 22 L 64 28 C 50 22, 28 22, 16 34 Z" fill="#7d9ce0" stroke="{ink}" stroke-width="2.6" stroke-linejoin="round"/>'
    '<circle cx="70" cy="22" r="5" fill="#fffaf0" stroke="{ink}" stroke-width="2.2"/>'
    '<path d="M33 58 Q 40 62 47 58" fill="none" stroke="{ink}" stroke-width="2.4" stroke-linecap="round"/>')
icons["portrait_villager"] = face(SKIN, "#6f4a35",
    '<path d="M13 40 C 12 18, 68 18, 67 40 C 60 30, 22 28, 13 40 Z" fill="{hair}" stroke="{ink}" stroke-width="2.6" stroke-linejoin="round"/>'
    '<path d="M33 58 Q 40 63 47 58" fill="none" stroke="{ink}" stroke-width="2.4" stroke-linecap="round"/>')

# map marks: small and plain, drawn in ink on the parchment
M = st("2.4")
def mark(body):
    return svg(f'<circle cx="16" cy="16" r="14" fill="{PAPER}" {st("2")}/>' + body, view="0 0 32 32")
icons["place_village"] = mark(f'<path d="M7 16 L 13 10 L 19 16 L 19 23 L 7 23 Z" fill="{ACC}" {M}/><path d="M15 14 L 20 9 L 26 15 L 26 23 L 19 23" fill="{WOOD}" {M}/>')
icons["place_camp"] = mark(f'<path d="M6 24 L 16 7 L 26 24 Z" fill="{ACC_D}" {M}/><path d="M16 7 L 16 24 M 13 24 L 16 17 L 19 24" fill="none" {M}/>')
icons["place_hut"] = mark(f'<path d="M5 25 Q 16 18 27 25" fill="{LEAF}" {M}/><path d="M10 21 L 16 12 L 22 21 Z" fill="{WOOD}" {M}/>')
icons["place_shrine"] = mark(f'<path d="M6 11 L 26 11 M 8 15 L 24 15 M 10 11 L 10 25 M 22 11 L 22 25" fill="none" {st("3", ACC_D)}/>')
icons["place_bridge"] = mark(f'<path d="M5 20 Q 16 10 27 20 L 27 23 L 5 23 Z" fill="{WOOD}" {M}/><path d="M5 25 Q 10 27 16 25 Q 22 23 27 25" fill="none" stroke="{SKY_D}" stroke-width="2"/>')
icons["place_tree"] = mark(f'<path d="M11 26 L 13 8 L 19 8 L 21 26 Z" fill="{WOOD_D}" {M}/><ellipse cx="16" cy="16" rx="2.4" ry="3.4" fill="{INK}"/>')
icons["place_scarecrow"] = mark(f'<path d="M16 12 L 16 27 M 7 15 L 25 15" fill="none" {M}/><circle cx="16" cy="9" r="3.6" fill="{LAMP}" {M}/><path d="M11 15 L 21 15 L 19 22 L 13 22 Z" fill="{SKY}" {M}/>')
icons["place_chest"] = mark(f'<rect x="7" y="12" width="18" height="12" rx="2" fill="{WOOD}" {M}/><path d="M7 17 L 25 17" {M}/><rect x="14.5" y="15" width="3" height="5" fill="{GOLD}" {st("1.4")}/>')
icons["place"] = mark(f'<polygon points="16,6 24,16 16,26 8,16" fill="{ACC}" {M}/>')

OUT.mkdir(parents=True, exist_ok=True)
for name, text in icons.items():
    (OUT / f"{name}.svg").write_text(text)
print(len(icons), "icons")
