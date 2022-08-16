import imgkit
from http.server import BaseHTTPRequestHandler, HTTPServer
from cgi import FieldStorage

class HtmlToImgServer(BaseHTTPRequestHandler):
    def do_POST(self):
        form = FieldStorage(
                fp = self.rfile,
                headers = self.headers,
                environ = { "REQUEST_METHOD": "POST" }
        )

        data = form.getvalue("html")
        options = { "format": "png", "quiet": "" }
        img = imgkit.from_string(data, False, options)

        self.send_response(200)
        self.send_header("Content-Type", "image/png")
        self.send_header("Content-Length", len(img))
        self.end_headers()
        self.wfile.write(img)

hostName = "localhost"
serverPort = 7227

webServer = HTTPServer((hostName, serverPort), HtmlToImgServer)
print(f"Server started http://{hostName}:{serverPort}")

try:
    webServer.serve_forever()
except KeyboardInterrupt:
    pass

webServer.server_close()
print("Server stopped")

