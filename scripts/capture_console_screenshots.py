# -*- coding: utf-8 -*-
"""Capture live Eventide console screenshots into docs/screenshots/.

Prerequisites:
  - eventide listening on BASE_URL (default http://127.0.0.1:8080)
  - pip install playwright && python -m playwright install chromium

Usage:
  set EVENTIDE_USER=admin
  set EVENTIDE_PASS=your-password
  python scripts/capture_console_screenshots.py
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

from playwright.sync_api import sync_playwright

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "docs" / "screenshots"
BASE = os.environ.get("EVENTIDE_BASE", "http://127.0.0.1:8080").rstrip("/")
USER = os.environ.get("EVENTIDE_USER", "admin")
PASS = os.environ.get("EVENTIDE_PASS", "admin123")


def shot(page, name: str):
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / name
    page.screenshot(path=str(path), full_page=False)
    print("saved", path.relative_to(ROOT))


def main() -> int:
    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page(viewport={"width": 1400, "height": 900})
        page.goto(f"{BASE}/", wait_until="domcontentloaded", timeout=30000)
        page.wait_for_timeout(500)
        shot(page, "01-login.png")

        page.fill("#username", USER)
        page.fill("#password", PASS)
        page.click("#login-form button[type=submit]")
        page.wait_for_timeout(2000)
        if page.locator("#view-login").is_visible():
            err = page.locator("#login-error").inner_text()
            print("login failed:", err or "(no error text)", file=sys.stderr)
            browser.close()
            return 1

        shot(page, "02-overview.png")

        for page_id, fname in [
            ("alerts", "03-alerts.png"),
            ("ingress", "04-ingress.png"),
            ("channels", "06-channels.png"),
            ("settings", "07-settings.png"),
            ("trap", "08-trap.png"),
        ]:
            page.click(f'.nav-item[data-page="{page_id}"]')
            page.wait_for_timeout(1200)
            if page_id == "ingress":
                help_btn = page.locator("#btn-ingress-help")
                if help_btn.count():
                    help_btn.click()
                    page.wait_for_timeout(400)
                details = page.locator("#ingress-help")
                if details.count():
                    page.evaluate(
                        "el => { el.open = true; }", details.element_handle()
                    )
                    page.wait_for_timeout(300)
                    shot(page, "05-ingress-help-live.png")
            shot(page, fname)

        browser.close()
    print("done — regenerate manual: python scripts/gen_user_manual_docx.py")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
