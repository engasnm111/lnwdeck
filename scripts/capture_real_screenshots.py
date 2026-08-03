import os
import time
import http.server
import socketserver
import threading
from playwright.sync_api import sync_playwright

DIST_DIR = r"E:\lnwdeck\apps\desktop\dist"
PORT = 4174

class SPAHandler(http.server.SimpleHTTPRequestHandler):
    """Serves index.html for any non-file route (SPA fallback)."""
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIST_DIR, **kwargs)

    def do_GET(self):
        # If the path corresponds to a real file, serve it normally
        file_path = os.path.join(DIST_DIR, self.path.lstrip("/"))
        if os.path.isfile(file_path):
            return super().do_GET()
        # Otherwise, serve index.html (SPA fallback)
        self.path = "/index.html"
        return super().do_GET()

def start_server():
    with socketserver.TCPServer(("127.0.0.1", PORT), SPAHandler) as httpd:
        httpd.serve_forever()

server_thread = threading.Thread(target=start_server, daemon=True)
server_thread.start()
time.sleep(1)

screenshots_dir = r"E:\lnwdeck\assets\screenshots"
os.makedirs(screenshots_dir, exist_ok=True)

with sync_playwright() as p:
    browser = p.chromium.launch(headless=True)
    page = browser.new_page(viewport={"width": 1920, "height": 1080})

    # 1. Overview Dashboard (index route "/")
    page.goto(f"http://127.0.0.1:{PORT}/")
    page.wait_for_load_state("networkidle")
    page.wait_for_timeout(2000)
    page.screenshot(path=os.path.join(screenshots_dir, "overview_dashboard.png"))
    print(f"Captured overview_dashboard.png ({os.path.getsize(os.path.join(screenshots_dir, 'overview_dashboard.png'))} bytes)")

    # 2. Providers Page ("/providers")
    page.goto(f"http://127.0.0.1:{PORT}/providers")
    page.wait_for_load_state("networkidle")
    page.wait_for_timeout(2000)
    page.screenshot(path=os.path.join(screenshots_dir, "providers_page.png"))
    print(f"Captured providers_page.png ({os.path.getsize(os.path.join(screenshots_dir, 'providers_page.png'))} bytes)")

    # 3. System Diagnostics ("/system")
    page.goto(f"http://127.0.0.1:{PORT}/system")
    page.wait_for_load_state("networkidle")
    page.wait_for_timeout(2000)
    page.screenshot(path=os.path.join(screenshots_dir, "system_diagnostics.png"))
    print(f"Captured system_diagnostics.png ({os.path.getsize(os.path.join(screenshots_dir, 'system_diagnostics.png'))} bytes)")

    browser.close()

print("All real screenshots captured successfully!")
