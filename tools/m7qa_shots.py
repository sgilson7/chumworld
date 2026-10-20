#!/usr/bin/env python3
"""M7 (task P3): every screen at 1280 x 720, 1920 x 1080 and 2560 x 1440,
saved as OUT/<screen>-<w>x<h>.png, with the layout check's findings
(tests/browser/layout.py) and the keyboard focus printed for each. Not a
check: it prints what it finds. The browser check runs the same layout and
focus checks and fails on them.

    .venv/bin/python tools/m7qa_shots.py OUT_DIR [--size 1920x1080]

Chromium with a hardware GPU (--use-angle=metal), as tools/shoot.py.
"""
import functools
import http.server
import pathlib
import sys
import threading

from playwright.sync_api import sync_playwright

ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tests" / "browser"))
import layout  # noqa: E402


class Quiet(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, ".wasm": "application/wasm", ".js": "text/javascript"}

    def log_message(self, *args):
        pass


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    out = pathlib.Path(args[0] if args else "shots/m7qa/sizes")
    out.mkdir(parents=True, exist_ok=True)
    sizes = layout.SIZES
    if "--size" in sys.argv:
        w, h = sys.argv[sys.argv.index("--size") + 1].split("x")
        sizes = [(int(w), int(h))]
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), functools.partial(Quiet, directory=str(ROOT / "dist")))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{server.server_address[1]}/index.html"
    total = 0
    with sync_playwright() as p:
        browser = p.chromium.launch(args=["--use-angle=metal", "--enable-gpu"])
        for w, h in sizes:
            page = browser.new_page(viewport={"width": w, "height": h}, device_scale_factor=1)
            for name, roots, tab in layout.screens(page, base):
                found = layout.problems(page, roots)
                page.screenshot(path=str(out / f"{name}-{w}x{h}.png"))
                focus = layout.tab_focus(page, roots) if tab else []
                bad_focus = [f["what"] for f in focus if not f["ring"]]
                total += len(found) + len(bad_focus)
                print(f"{name} {w}x{h}: {len(found)} layout, {len(bad_focus)} focus")
                for f in found:
                    print("    " + f)
                for f in bad_focus:
                    print("    focus shows no ring: " + f)
            page.close()
        browser.close()
    server.shutdown()
    print(f"\n{total} findings")


if __name__ == "__main__":
    main()
