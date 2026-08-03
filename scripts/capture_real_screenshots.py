import os
import time
import http.server
import socketserver
import threading
from playwright.sync_api import sync_playwright

DIST_DIR = r"E:\lnwdeck\apps\desktop\dist"
PORT = 4173

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIST_DIR, **kwargs)

def start_server():
    with socketserver.TCPServer(("127.0.0.1", PORT), Handler) as httpd:
        httpd.serve_forever()

server_thread = threading.Thread(target=start_server, daemon=True)
server_thread.start()

time.sleep(1)

screenshots_dir = r"E:\lnwdeck\assets\screenshots"
os.makedirs(screenshots_dir, exist_ok=True)

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True)
    page = browser.new_page(viewport={"width": 1920, "height": 1080})

    # Capture Overview Dashboard
    page.goto(f"http://127.0.0.1:{PORT}/#/")
    page.wait_for_selector(".metrics-grid", timeout=10000)
    page.wait_for_timeout(1000)
    page.screenshot(path=os.path.join(screenshots_dir, "overview_dashboard.png"))
    print("Captured real screenshot: overview_dashboard.png")

    # Capture Providers Page
    page.evaluate("window.location.hash = '#/providers'")
    page.wait_for_timeout(1000)
    page.screenshot(path=os.path.join(screenshots_dir, "providers_page.png"))
    print("Captured real screenshot: providers_page.png")

    # Capture System Diagnostics Page
    page.evaluate("window.location.hash = '#/system'")
    page.wait_for_timeout(1000)
    page.screenshot(path=os.path.join(screenshots_dir, "system_diagnostics.png"))
    print("Captured real screenshot: system_diagnostics.png")

    browser.close()

print("Real app screenshots captured successfully!")
