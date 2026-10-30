"""Browser checks of the web app, with mock data only. Needs a running chain, see apps/web/README.md.

    python apps/web/tests/smoke.py [http://localhost:8080] [--production]

--production: the app is served by rolxapp-webhost itself, so any console error counts. Under
`dx serve`, the dev page's own script and websocket errors are tolerated.

The host needs a Google client id, any value, so that the app shows the sign-in button; Google's
script is replaced by a fake here.
"""

import json
import sys
from datetime import datetime, timedelta, timezone

from playwright.sync_api import Error as PlaywrightError
from playwright.sync_api import Page, expect, sync_playwright

DEV_PAGE_NOISE = ("inline script", "WebSocket", "ws://", "_dioxus")
SESSION_KEY = "rolxapp.session"

# Stands in for Google's script: draws a button that reports a mock id token when clicked.
FAKE_GOOGLE = """
window.google = { accounts: { id: {
    initialize(options) {
        window.fakeGoogleInitialized = (window.fakeGoogleInitialized || 0) + 1;
        window.fakeGoogleClientId = options.client_id;
        window.fakeGoogleCallback = options.callback;
    },
    renderButton(element) {
        const button = document.createElement("button");
        button.id = "fake-google-button";
        button.textContent = "Fake Google";
        button.onclick = () => window.fakeGoogleCallback({ credential: CREDENTIAL });
        element.appendChild(button);
    },
} } };
"""

def mock_session(hours: float = 5) -> str:
    expires = datetime.now(timezone.utc) + timedelta(hours=hours)
    return json.dumps(
        {
            "bearer_token": f"mock-bearer-{int(expires.timestamp())}",
            "expires": expires.strftime("%Y-%m-%dT%H:%M:%SZ"),
            "user_id": "00000000-0000-4000-8000-000000000001",
            "user_email": "mock.user@example.invalid",
            "user_first_name": "Mock",
            "user_last_name": "User",
        }
    )


def use_fake_google(context, credential: str):
    script = FAKE_GOOGLE.replace("CREDENTIAL", json.dumps(credential))
    context.route(
        "https://accounts.google.com/gsi/client",
        lambda route: route.fulfill(content_type="text/javascript", body=script),
    )


def open_app(browser, url: str, session: str | None) -> tuple[Page, list[str]]:
    context = browser.new_context(timezone_id="Europe/Zurich")
    if session:
        # Once only: a reload must see what the app left in the store.
        context.add_init_script(
            f"if (!sessionStorage.getItem('seeded')) {{"
            f" sessionStorage.setItem('seeded', '1');"
            f" localStorage.setItem({json.dumps(SESSION_KEY)}, {json.dumps(session)}); }}"
        )
    # These checks are not about the button, and must not depend on real Google.
    use_fake_google(context, "mock-id-token")
    page = context.new_page()
    errors: list[str] = []
    page.on("pageerror", lambda e: errors.append(str(e)))
    page.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    page.goto(url)
    return page, errors


def stored_session(page: Page):
    return page.evaluate(f"localStorage.getItem({json.dumps(SESSION_KEY)})")


def check_login_without_a_session(browser, url):
    page, errors = open_app(browser, url, None)
    expect(page.get_by_text("Sign in to see your time records.")).to_be_visible(timeout=30_000)
    return errors


def go_to_a_weekday(page: Page):
    """Today may be a weekend, on which the mock has no entries."""
    for _ in range(3):
        expect(page.locator("#entries").or_(page.locator("#empty"))).to_be_visible(timeout=15_000)
        if page.locator("#entries").count() > 0:
            return
        page.locator("#previous-day").click()
    raise AssertionError("no day with entries found")


def check_day_navigation_and_logout(browser, url):
    page, errors = open_app(browser, url, mock_session())
    expect(page.locator("#user-name")).to_have_text("Mock User", timeout=30_000)
    expect(page.locator("#loading")).to_have_count(0, timeout=15_000)
    go_to_a_weekday(page)

    expect(page.locator("#entries")).to_contain_text("M0001.001.01")
    expect(page.locator("#total")).to_contain_text("Total")
    first_date = page.locator("#date").inner_text()
    first_comment = page.locator("#entries").inner_text()
    assert f"Mock work on {first_date}" in first_comment, first_comment

    page.locator("#next-day").click()
    expect(page.locator("#date")).not_to_have_text(first_date)
    next_date = page.locator("#date").inner_text()
    # The content has to follow the heading: entries of that very day, or none on a weekend.
    expect(
        page.get_by_text(f"Mock work on {next_date}").or_(page.locator("#empty"))
    ).to_be_visible(timeout=15_000)
    expect(page.get_by_text(f"Mock work on {first_date}")).to_have_count(0)

    page.locator("#previous-day").click()
    expect(page.locator("#date")).to_have_text(first_date)
    expect(page.locator("#entries")).to_contain_text(f"Mock work on {first_date}", timeout=15_000)

    assert stored_session(page) is not None
    page.locator("#logout").click()
    expect(page.get_by_text("Sign in to see your time records.")).to_be_visible()
    assert stored_session(page) is None, "logout must clear the storage"

    page.reload()
    expect(page.get_by_text("Sign in to see your time records.")).to_be_visible(timeout=30_000)
    return errors





def open_app_with_fake_google(browser, url, credential):
    context = browser.new_context()
    use_fake_google(context, credential)
    page = context.new_page()
    errors: list[str] = []
    page.on("pageerror", lambda e: errors.append(str(e)))
    page.on("console", lambda m: errors.append(m.text) if m.type == "error" else None)
    page.goto(url)
    return page, errors


def check_google_button_signs_in(browser, url):
    page, errors = open_app_with_fake_google(browser, url, "mock-id-token")
    page.locator("#fake-google-button").click(timeout=30_000)

    expect(page.locator("#user-name")).to_have_text("Mock User", timeout=15_000)
    assert stored_session(page) is not None, "the session must be saved"
    assert page.evaluate("window.fakeGoogleClientId"), "the client id must reach Google's API"

    # A new login screen after logout needs a working button, and Google only one initialize.
    page.locator("#logout").click()
    page.locator("#fake-google-button").click(timeout=15_000)
    expect(page.locator("#user-name")).to_have_text("Mock User", timeout=15_000)
    initialized = page.evaluate("window.fakeGoogleInitialized")
    assert initialized == 1, f"initialize was called {initialized} times"
    return errors


def check_google_button_reports_a_refused_token(browser, url):
    page, errors = open_app_with_fake_google(browser, url, "refused-by-the-server")
    page.locator("#fake-google-button").click(timeout=30_000)

    expect(page.locator("#login-error")).to_contain_text("Sign-in failed", timeout=15_000)
    assert stored_session(page) is None
    expect(page.locator("#fake-google-button")).to_be_visible()
    rejected = [e for e in errors if "status of 401" in e]
    assert rejected, "expected the server to answer 401"
    return [e for e in errors if e not in rejected]


def check_rejected_session_returns_to_the_login(browser, url):
    session = json.loads(mock_session())
    session["bearer_token"] = "rejected-token"
    page, errors = open_app(browser, url, json.dumps(session))
    expect(page.get_by_text("Sign in to see your time records.")).to_be_visible(timeout=30_000)
    assert stored_session(page) is None
    # The browser itself reports the 401, which also proves the request got that far.
    rejected = [e for e in errors if "status of 401" in e]
    assert rejected, "expected the server to answer 401"
    return [e for e in errors if e not in rejected]


def main():
    args = [a for a in sys.argv[1:] if a != "--production"]
    url = args[0] if args else "http://localhost:8080"
    production = "--production" in sys.argv

    checks = [
        check_login_without_a_session,
        check_day_navigation_and_logout,
        check_rejected_session_returns_to_the_login,
        check_google_button_signs_in,
        check_google_button_reports_a_refused_token,
    ]
    failed = 0
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch(channel="msedge")
        for check in checks:
            try:
                errors = check(browser, url)
                if not production:
                    errors = [e for e in errors if not any(n in e for n in DEV_PAGE_NOISE)]
                assert not errors, f"console errors: {[e[:200] for e in errors[:3]]}"
                print(f"ok      {check.__name__}")
            except (AssertionError, PlaywrightError) as error:
                failed += 1
                print(f"FAILED  {check.__name__}: {str(error)[:600]}")
        browser.close()
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
