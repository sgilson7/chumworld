"""M7: every screen's words laid out at three window sizes, and the keyboard's
focus always visible (R9.86). Used by interactions.py in every engine and by
tools/m7qa_shots.py for the screenshots.

The layout check reads each visible text node's boxes on the screen in
front (a ring face turned away is skipped: it is drawn at the side on
purpose, as Ocarina of Time's are), and reports:

- words off the window;
- words cut by the nearest box that clips its overflow (a list that scrolls
  counts: a list that needs scrolling at a size is one that does not fit);
- an element whose words are wider than it and hidden (an ellipsis);
- words of one element overlapping the words of another;
- a key cap or a button's box overlapping words outside it.

The focus check moves the keyboard's focus with Tab and asks of each
focused element that it shows a ring: an outline, or a box shadow that the
focus added.

Expected values here are layout facts, never the game's own numbers.
"""

LAYOUT = r"""(roots) => {
    const out = [];
    const shown = (el) => {
        for (let e = el; e; e = e.parentElement) {
            const s = getComputedStyle(e);
            if (s.display === 'none' || s.visibility === 'hidden' || parseFloat(s.opacity) < 0.05) return false;
            if (e.classList && e.classList.contains('face') && !e.classList.contains('front')) return false;
            if (e.hidden) return false;
        }
        return true;
    };
    const texts = [];
    const caps = [];
    for (const sel of roots) {
        for (const root of document.querySelectorAll(sel)) {
            if (!shown(root)) continue;
            const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
            for (let n = walk.nextNode(); n; n = walk.nextNode()) {
                const t = n.textContent.trim();
                if (!t) continue;
                const el = n.parentElement;
                if (!shown(el)) continue;
                const r = document.createRange();
                r.selectNodeContents(n);
                for (const b of r.getClientRects()) {
                    if (b.width > 0.5 && b.height > 0.5) texts.push({el, b, t: t.slice(0, 48)});
                }
            }
            for (const el of root.querySelectorAll('kbd, button')) {
                if (shown(el)) caps.push({el, b: el.getBoundingClientRect()});
            }
        }
    }
    const meet = (a, b) => Math.max(0, Math.min(a.right, b.right) - Math.max(a.left, b.left))
        * Math.max(0, Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top));
    const name = (el) => (el.id ? '#' + el.id : '') + (el.className && el.className.baseVal === undefined ? '.' + String(el.className).split(' ').join('.') : '') || el.tagName;
    const seen = new Set();
    const say = (s) => { if (!seen.has(s)) { seen.add(s); out.push(s); } };
    for (const {el, b, t} of texts) {
        if (b.left < -1 || b.top < -1 || b.right > innerWidth + 1 || b.bottom > innerHeight + 1) say(`off the window: "${t}"`);
        for (let e = el; e && e !== document.body; e = e.parentElement) {
            const s = getComputedStyle(e);
            if (s.display === 'contents') continue;
            if (s.overflowX !== 'visible' || s.overflowY !== 'visible') {
                const c = e.getBoundingClientRect();
                if (b.left < c.left - 1 || b.right > c.right + 1 || b.top < c.top - 1 || b.bottom > c.bottom + 1) say(`cut by ${name(e)}: "${t}"`);
                if (e.scrollHeight > e.clientHeight + 1 && s.overflowY !== 'visible') say(`${name(e)} needs scrolling: "${t}"`);
                break;
            }
        }
        if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflowX === 'hidden') say(`hidden past its width: "${t}"`);
    }
    for (let i = 0; i < texts.length; i++) {
        for (let j = i + 1; j < texts.length; j++) {
            const a = texts[i], c = texts[j];
            if (a.el === c.el || a.el.contains(c.el) || c.el.contains(a.el)) continue;
            if (meet(a.b, c.b) > 4) say(`words overlap: "${a.t}" and "${c.t}"`);
        }
    }
    for (const {el, b} of caps) {
        for (const x of texts) {
            if (el.contains(x.el) || x.el.contains(el)) continue;
            if (meet(b, x.b) > 4) say(`${el.tagName.toLowerCase()} "${el.textContent.trim().slice(0, 24)}" covers "${x.t}"`);
        }
    }
    return out;
}"""

# Whether the focused element shows a ring: an outline, or a box shadow
# other than the one it has unfocused.
FOCUS = r"""() => {
    const el = document.activeElement;
    if (!el || el === document.body) return {what: 'nothing', ring: false};
    const s = getComputedStyle(el);
    const outline = s.outlineStyle !== 'none' && parseFloat(s.outlineWidth) >= 2;
    const what = (el.id || el.className || el.tagName) + ' ' + (el.textContent || '').trim().slice(0, 24);
    if (outline) return {what, ring: true};
    const focused = s.boxShadow;
    el.blur();
    const plain = getComputedStyle(el).boxShadow;
    el.focus({preventScroll: true});
    return {what, ring: focused !== plain && focused !== 'none'};
}"""

SIZES = [(1280, 720), (1920, 1080), (2560, 1440)]

# Waits until CSS animations and transitions on the page have ended, and two
# more frames.
SETTLE = """() => new Promise((done) => {
    const check = () => {
        const busy = document.getAnimations().some((a) => a.playState === 'running' && a.effect && a.effect.getComputedTiming().iterations !== Infinity);
        if (busy) requestAnimationFrame(check);
        else requestAnimationFrame(() => requestAnimationFrame(done));
    };
    requestAnimationFrame(check);
})"""


def settle(page):
    page.evaluate(SETTLE)


def problems(page, roots):
    settle(page)
    return page.evaluate(LAYOUT, roots)


COUNT = r"""(roots) => roots.flatMap((r) => [...document.querySelectorAll(r + ' button, ' + r + ' input, ' + r + ' [tabindex]')])
    .filter((el) => el.offsetParent !== null && !el.disabled && getComputedStyle(el).visibility !== 'hidden').length"""


def tab_focus(page, roots):
    """Presses Tab once more than there are controls on the screen, so the
    focus passes every one (and once past the last, onto the page itself,
    which is not counted); returns each focused control and whether it
    shows a ring."""
    n = page.evaluate(COUNT, roots)
    seen = []
    for _ in range(n + 1):
        page.keyboard.press("Tab")
        settle(page)
        f = page.evaluate(FOCUS)
        if f["what"] != "nothing":
            seen.append(f)
    return seen


# The screens, each as (name, how to reach it from the one before, the
# selectors whose words are checked, whether Tab moves focus there).
def screens(page, base):
    """Walks every screen in order, yielding (name, roots, tab) on each, the
    page at that screen. The page opens with the test inventory and HUD."""
    page.goto(base + "?pointer=free&act=test.give_all&act=test.hud&act=test.explore&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    yield "title", ["#overlay"], True
    page.click("#title-more button")
    page.wait_for_selector("#settings.shown")
    yield "settings", ["#settings"], True
    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='title']", state="attached")
    page.click("#title button")
    page.wait_for_selector("body[data-screen='play']", state="attached")
    page.wait_for_selector("#play-hud.shown")
    yield "hud", ["#play-hud", "#hud"], False
    page.keyboard.press("Tab")
    page.wait_for_selector("#menu.shown")
    for face in ["select", "map", "quest", "equipment"]:
        page.wait_for_selector(f"#menu[data-face='{face}']")
        yield f"menu_{face}", ["#menu"], False
        page.keyboard.press("KeyE")
    page.wait_for_selector("#menu[data-face='select']")
    page.hover("#menu .menu-foot .quick-row .quick >> nth=1")
    page.wait_for_selector("#menu .quick-info:not([hidden])")
    yield "menu_quick", ["#menu"], False
    page.mouse.move(1, 1)
    page.keyboard.press("Escape")
    page.wait_for_selector("body[data-screen='paused']", state="attached")
    yield "paused", ["#overlay"], True
    page.goto(base + "?pointer=free&act=test.hud&act=test.dialogue&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("#talk.shown")
    page.wait_for_function("() => document.querySelector('#talk .talk-next.done')")
    yield "dialogue", ["#talk", "#play-hud"], False
    page.goto(base + "?pointer=free&act=test.shop&act=inventory.give:sword&act=inventory.give:jacket&act=story.still:on")
    page.wait_for_selector("body[data-ready='true']", state="attached", timeout=60000)
    page.click("#title button")
    page.wait_for_selector("#shop.shown")
    page.click("#shop .shop-row[data-key='short_sword'] .row-button")
    page.wait_for_selector("#shop .shop-note:not(:empty)")
    yield "shop", ["#shop"], True
