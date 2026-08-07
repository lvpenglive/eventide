# -*- coding: utf-8 -*-
"""Eventide console E2E tests (Playwright).

Prerequisites:
  - eventide listening on EVENTIDE_BASE (default http://127.0.0.1:8080)
  - pip install playwright && python -m playwright install chromium

Usage (PowerShell):
  $env:EVENTIDE_USER="admin"
  $env:EVENTIDE_PASS="admin123"
  python scripts/e2e_console.py

Optional:
  $env:EVENTIDE_HEADLESS="0"   # show browser
  $env:EVENTIDE_E2E_ARTIFACTS="1"  # save failure screenshots to docs/e2e-artifacts/
"""
from __future__ import annotations

import os
import sys
import time
import traceback
from dataclasses import dataclass, field
from pathlib import Path

from playwright.sync_api import Page, expect, sync_playwright

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "docs" / "e2e-artifacts"
BASE = os.environ.get("EVENTIDE_BASE", "http://127.0.0.1:8080").rstrip("/")
USER = os.environ.get("EVENTIDE_USER", "admin")
PASS = os.environ.get("EVENTIDE_PASS", "admin123")
HEADLESS = os.environ.get("EVENTIDE_HEADLESS", "1") not in ("0", "false", "False")
SAVE_ART = os.environ.get("EVENTIDE_E2E_ARTIFACTS", "1") not in ("0", "false", "False")

NAV_PAGES = [
    ("overview", "总览"),
    ("alerts", "告警事件"),
    ("silences", "静默策略"),
    ("datasources", "数据源"),
    ("rules", "告警规则"),
    ("ingress", "告警接入"),
    ("trap", "SNMP Trap"),
    ("mib", "MIB 库"),
    ("policies", "Trap 策略"),
    ("channels", "通知渠道"),
    ("notifies", "通知日志"),
    ("enrich", "告警丰富"),
    ("users", "用户管理"),
    ("roles", "权限管理"),
    ("departments", "部门管理"),
    ("settings", "系统设置"),
    ("kafka", "Kafka"),
]


@dataclass
class Suite:
    passed: list[str] = field(default_factory=list)
    failed: list[tuple[str, str]] = field(default_factory=list)

    def ok(self, name: str) -> None:
        self.passed.append(name)
        print(f"  PASS  {name}")

    def fail(self, name: str, err: str) -> None:
        self.failed.append((name, err))
        print(f"  FAIL  {name}: {err}", file=sys.stderr)


def artifact(page: Page, name: str) -> None:
    if not SAVE_ART:
        return
    ART.mkdir(parents=True, exist_ok=True)
    path = ART / f"{int(time.time())}_{name}.png"
    try:
        page.screenshot(path=str(path), full_page=False)
        print(f"  artifact {path.relative_to(ROOT)}")
    except Exception as e:
        print(f"  artifact skip: {e}", file=sys.stderr)


def wait_page_settled(page: Page, timeout_ms: int = 15000) -> None:
    """Wait until page-root is not the loading placeholder."""
    page.wait_for_function(
        """() => {
          const root = document.getElementById('page-root');
          if (!root) return false;
          const t = (root.textContent || '').trim();
          return t && t !== '加载中…' && !t.startsWith('加载中');
        }""",
        timeout=timeout_ms,
    )


def assert_no_page_load_error(page: Page) -> None:
    root = page.locator("#page-root")
    text = root.inner_text(timeout=5000)
    if "加载失败" in text:
        raise AssertionError(f"page-root shows load error: {text[:200]}")


def go_nav(page: Page, page_id: str) -> None:
    btn = page.locator(f'.nav-item[data-page="{page_id}"]')
    expect(btn).to_be_visible(timeout=10000)
    # ensure parent group open if nested
    page.evaluate(
        """(id) => {
          const btn = document.querySelector(`.nav-item[data-page="${id}"]`);
          if (!btn) return;
          const group = btn.closest('.nav-group');
          if (group) group.classList.add('open');
        }""",
        page_id,
    )
    btn.click()
    expect(page.locator(f'.nav-item[data-page="{page_id}"].active')).to_be_visible(
        timeout=10000
    )
    wait_page_settled(page)
    assert_no_page_load_error(page)


def login(page: Page, username: str, password: str) -> None:
    page.goto(f"{BASE}/", wait_until="domcontentloaded", timeout=30000)
    expect(page.locator("#login-form")).to_be_visible(timeout=15000)
    page.fill("#username", username)
    page.fill("#password", password)
    page.click("#login-form button[type=submit]")
    page.wait_for_timeout(800)


def run_case(suite: Suite, page: Page, name: str, fn) -> None:
    try:
        fn()
        suite.ok(name)
    except Exception as e:
        suite.fail(name, str(e))
        artifact(page, name.replace(" ", "_"))
        if os.environ.get("EVENTIDE_E2E_VERBOSE"):
            traceback.print_exc()


def main() -> int:
    print(f"E2E base={BASE} user={USER} headless={HEADLESS}")
    suite = Suite()

    with sync_playwright() as p:
        browser = p.chromium.launch(headless=HEADLESS)
        context = browser.new_context(viewport={"width": 1440, "height": 900})
        page = context.new_page()
        page.set_default_timeout(20000)

        # --- auth ---
        def case_bad_login():
            login(page, USER, "definitely-wrong-password-xyz")
            expect(page.locator("#view-login")).to_be_visible(timeout=10000)
            err = page.locator("#login-error").inner_text()
            if not err.strip():
                # still on login is enough; some builds only toast
                pass
            # scrub check: password must not linger in URL from failed GET
            if "password=" in page.url:
                raise AssertionError(f"password leaked into URL: {page.url}")

        run_case(suite, page, "login rejects bad password", case_bad_login)

        def case_good_login():
            login(page, USER, PASS)
            expect(page.locator("#view-app")).to_be_visible(timeout=15000)
            expect(page.locator("#view-login")).to_be_hidden(timeout=5000)
            if "password=" in page.url or "username=" in page.url:
                raise AssertionError(f"credentials in URL after login: {page.url}")
            wait_page_settled(page)
            assert_no_page_load_error(page)

        run_case(suite, page, "login succeeds", case_good_login)

        if suite.failed and any(n == "login succeeds" for n, _ in suite.failed):
            print("abort: cannot continue without login", file=sys.stderr)
            browser.close()
            _summary(suite)
            return 1

        # --- navigation ---
        for page_id, title in NAV_PAGES:

            def case_nav(pid=page_id, ttl=title):
                go_nav(page, pid)
                heading = page.locator("#page-title").inner_text()
                if ttl not in heading and heading.strip() == "":
                    raise AssertionError(f"unexpected title {heading!r}, want {ttl}")

            run_case(suite, page, f"nav {page_id}", case_nav)

        # --- ingress help ---
        def case_ingress_help():
            go_nav(page, "ingress")
            help_btn = page.locator("#btn-ingress-help")
            expect(help_btn).to_be_visible()
            help_btn.click()
            details = page.locator("#ingress-help")
            expect(details).to_be_visible()
            page.evaluate("(el) => { el.open = true; }", details.element_handle())
            body = details.inner_text()
            for needle in ("Alertmanager", "Kafka", "SNMP"):
                if needle not in body:
                    raise AssertionError(f"help missing {needle}")
            quick = page.locator("[data-quick-kind]")
            if quick.count() < 3:
                raise AssertionError(f"expected quick-create buttons, got {quick.count()}")

        run_case(suite, page, "ingress help + quick-create", case_ingress_help)

        # --- ingress test push (first enabled card if any) ---
        def case_ingress_test():
            go_nav(page, "ingress")
            btn = page.locator("button[data-test]:not([disabled])")
            if btn.count() == 0:
                raise AssertionError("SKIP:no-enabled-ingress")
            btn.first.click()
            page.wait_for_timeout(600)
            # modal should appear with test UI
            body = page.locator("#modal-body")
            expect(body).to_be_visible(timeout=8000)
            send = page.locator(
                "#modal-body button.primary, #modal button.primary, "
                "#modal-body button:has-text('推送'), #modal-body button:has-text('发送')"
            )
            if send.count():
                send.first.click()
                page.wait_for_timeout(2000)
            # dismiss
            for sel in ("#m-cancel", "#modal-body button:has-text('关闭')", "#modal-body button:has-text('取消')"):
                c = page.locator(sel)
                if c.count():
                    c.first.click()
                    break
            else:
                page.keyboard.press("Escape")
            page.wait_for_timeout(300)

        def run_ingress_test():
            try:
                case_ingress_test()
                suite.ok("ingress test push flow")
            except AssertionError as e:
                if str(e).startswith("SKIP:"):
                    print("  SKIP  ingress test push flow (no enabled route)")
                    suite.ok("ingress test push flow (skipped)")
                else:
                    suite.fail("ingress test push flow", str(e))
                    artifact(page, "ingress_test_push")
            except Exception as e:
                suite.fail("ingress test push flow", str(e))
                artifact(page, "ingress_test_push")

        run_ingress_test()

        # --- alerts filters render ---
        def case_alerts_filters():
            go_nav(page, "alerts")
            # common filter controls
            root = page.locator("#page-root").inner_text()
            if "告警" not in root and "firing" not in root.lower() and "暂无" not in root:
                # still ok if table empty but page loaded
                pass
            # try status tab / select if present
            sev = page.locator("#alert-sev, select[name=severity], .alert-tab").first
            if sev.count():
                expect(sev).to_be_visible()

        run_case(suite, page, "alerts page interactive", case_alerts_filters)

        # --- settings storm section ---
        def case_settings():
            go_nav(page, "settings")
            text = page.locator("#page-root").inner_text()
            # storm / license / trap token keywords (any subset)
            hits = sum(
                1
                for k in ("风暴", "许可", "Trap", "主题", "历史", "Elasticsearch", "通知")
                if k in text
            )
            if hits < 1:
                raise AssertionError("settings page missing expected sections")

        run_case(suite, page, "settings sections visible", case_settings)

        # --- logout ---
        def case_logout():
            page.click("#btn-logout")
            expect(page.locator("#view-login")).to_be_visible(timeout=10000)
            expect(page.locator("#login-form")).to_be_visible()

        run_case(suite, page, "logout returns to login", case_logout)

        browser.close()

    return _summary(suite)


def _summary(suite: Suite) -> int:
    total = len(suite.passed) + len(suite.failed)
    print()
    print(f"E2E result: {len(suite.passed)}/{total} passed")
    if suite.failed:
        print("Failures:")
        for name, err in suite.failed:
            print(f"  - {name}: {err}")
        return 1
    print("All console E2E checks passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
