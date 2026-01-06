import os
import time

import pytest
from appium import webdriver
from appium.options.common import AppiumOptions
from appium.webdriver.common.appiumby import AppiumBy
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import WebDriverWait


def _get_app_path() -> str:
    app_path = os.environ.get("RUST_SWITCHER_EXE")
    if not app_path:
        raise RuntimeError("RUST_SWITCHER_EXE must point to rust-switcher.exe")

    app_path = os.path.abspath(app_path)
    if not os.path.exists(app_path):
        raise RuntimeError(f"RUST_SWITCHER_EXE does not exist: {app_path}")

    return app_path


def _wait_visible(driver, by, value, timeout=30):
    return WebDriverWait(driver, timeout).until(EC.visibility_of_element_located((by, value)))


def _text_value(element) -> str:
    v = element.get_attribute("Value.Value")
    if v is None:
        v = element.get_attribute("Text")
    return "" if v is None else str(v)


def _set_text(element, text: str):
    element.click()
    element.send_keys(Keys.CONTROL, "a")
    element.send_keys(Keys.DELETE)
    element.send_keys(text)


@pytest.fixture(scope="module")
def driver():
    options = AppiumOptions()
    options.load_capabilities(
        {
            "platformName": "Windows",
            "deviceName": "WindowsPC",
            "app": _get_app_path(),
        }
    )

    server_url = os.environ.get("WINAPPDRIVER_URL", "http://127.0.0.1:4723/wd/hub")

    last_err = None
    drv = None
    for _attempt in range(20):
        try:
            drv = webdriver.Remote(server_url, options=options)
            break
        except Exception as e:
            last_err = e
            time.sleep(1)

    if drv is None:
        raise RuntimeError(f"Failed to create WinAppDriver session: {last_err}")

    try:
        yield drv
    finally:
        try:
            drv.close_app()
        except Exception:
            pass
        try:
            drv.quit()
        except Exception:
            pass


def test_smoke_main_window_and_controls(driver):
    main_window = _wait_visible(driver, AppiumBy.NAME, "RustSwitcher", timeout=60)
    assert main_window.is_displayed()

    delay_input = _wait_visible(driver, AppiumBy.ACCESSIBILITY_ID, "1003")
    apply_button = _wait_visible(driver, AppiumBy.NAME, "Apply")
    autoconvert_label = _wait_visible(driver, AppiumBy.NAME, "Autoconvert pause:")

    assert delay_input.is_displayed()
    assert apply_button.is_displayed()
    assert autoconvert_label.is_displayed()

    before = _text_value(delay_input)
    _set_text(delay_input, "250")

    WebDriverWait(driver, 10).until(lambda _d: _text_value(delay_input) != before)
    after = _text_value(delay_input)

    assert before != after

    cancel_button = _wait_visible(driver, AppiumBy.NAME, "Cancel")
    cancel_button.click()
