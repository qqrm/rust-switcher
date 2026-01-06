import os
import time

import pytest
from appium import webdriver
from appium.options.common import AppiumOptions
from appium.webdriver.common.appiumby import AppiumBy
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import WebDriverWait


def _get_app_path() -> str:
    app_path = os.environ.get("RUST_SWITCHER_EXE")
    if not app_path:
        raise RuntimeError("RUST_SWITCHER_EXE must point to the rust-switcher.exe")
    app_path = os.path.abspath(app_path)
    if not os.path.exists(app_path):
        raise RuntimeError(f"RUST_SWITCHER_EXE does not exist: {app_path}")
    return app_path


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
    driver = None
    for attempt in range(10):
        try:
            driver = webdriver.Remote(server_url, options=options)
            break
        except Exception:
            if attempt == 9:
                raise
            time.sleep(2)
    try:
        yield driver
    finally:
        try:
            driver.close_app()
        except Exception:
            pass
        if driver is not None:
            driver.quit()


def _wait_for(driver, by, value, timeout=20):
    return WebDriverWait(driver, timeout).until(
        EC.presence_of_element_located((by, value))
    )


def _text_value(element) -> str:
    return element.get_attribute("Value.Value")


def test_smoke_main_window_and_controls(driver):
    main_window = _wait_for(driver, AppiumBy.NAME, "RustSwitcher")
    assert main_window.is_displayed()

    delay_input = _wait_for(driver, AppiumBy.ACCESSIBILITY_ID, "1003")
    apply_button = _wait_for(driver, AppiumBy.NAME, "Apply")
    autoconvert_label = _wait_for(driver, AppiumBy.NAME, "Autoconvert pause:")

    assert delay_input.is_displayed()
    assert apply_button.is_displayed()
    assert autoconvert_label.is_displayed()

    before = _text_value(delay_input)
    delay_input.clear()
    delay_input.send_keys("250")
    after = _text_value(delay_input)

    assert before != after

    cancel_button = _wait_for(driver, AppiumBy.NAME, "Cancel")
    cancel_button.click()
