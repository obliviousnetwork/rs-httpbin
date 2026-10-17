# rs-httpbin

A copy of [go-httpbin](https://github.com/mccutchen/go-httpbin), Just for learning Rust language.

A rust port of the venerable httpbin.org HTTP request & response testing service.

## todo

- [x] [/](/) This page.
- [x] [/anything/:anything](/anything/anything) Returns request data.
- [x] [/delete](/delete) Returns request data. Allows only DELETE requests.
- [x] [/get](/get) Returns request data. Allows only GET requests.
- [x] [/head](/head) Returns request data. Allows only HEAD requests.
- [x] [/options](/options) Returns request data. Allows only OPTIONS requests.
- [x] [/patch](/patch) Returns request data. Allows only PATCH requests.
- [x] [/post](/post) Returns request data. Allows only POST requests.
- [x] [/put](/put) Returns request data. Allows only PUT requests.
- [x] [/trace](/trace) Returns request data. Allows only TRACE requests.
- [x] [/absolute-redirect/:n](/absolute-redirect/3) 302 Absolute redirects n times.
- [x] [/base64/:value](/base64/aGVsbG8K) Decodes a Base64-encoded string.
- [x] [/base64/decode/:value](/base64/decode/aGVsbG8K) Explicit URL for decoding a Base64 encoded string.
- [x] [/base64/encode/:value](/base64/encode/hello) Encodes a string into URL-safe Base64.
- [x] [/basic-auth/:user/:passwd](/basic-auth/user/passwd) Challenges HTTPBasic Auth.
- [x] [/bearer](/bearer) Checks Bearer token header - returns 401 if not set.
- [x] [/br](/br) Returns brotli-encoded data.
- [x] [/bytes/:n](/bytes/1024?seed=) Generates min(n, 10_000_000) random bytes of binary data, accepts optional seed integer parameter.
- [x] [/cache](/cache) Returns 200 unless an If-Modified-Since or If-None-Match header is provided, when it returns a 304.
- [x] [/cache/:n](/cache/3) Sets a Cache-Control header for n seconds.
- [x] [/cookies](/cookies) Returns cookie data.
- [x] [/cookies/delete?name](/cookies/delete?k1=&k2=) Deletes one or more simple cookies.
- [x] [/cookies/set?name=value](/cookies/set?k1=v1&k2=v2) Sets one or more simple cookies.
- [x] [/deflate](/deflate) Returns deflate-encoded data.
- [x] [/delay/:n](/delay/3) Delays responding for min(n, 10) seconds.
- [x] [/deny](/deny) Denied by robots.txt file.
- [x] [/digest-auth/:qop/:user/:passwd/:algorithm](/digest-auth/auth/user/password/SHA-256) Challenges HTTP Digest Auth.
- [x] [/digest-auth/:qop/:user/:passwd](/digest-auth/auth/user/password) Challenges HTTP Digest Auth.
- [x] [/drip?numbytes=n&duration=s&delay=s&code=code](/drip?numbytes=100&duration=2&delay=1&code=200) Drips data over a duration after an optional initial delay, then (optionally) returns with the given status code.
- [x] [/dump/request](/dump/request?a=1) Returns the given request in its HTTP/1.x wire approximate representation.
- [x] [/encoding/utf8](/encoding/utf8) Returns page containing UTF-8 data.
- [x] [/etag/:etag](/etag/etag) Assumes the resource has the given etag and responds to If-None-Match header with a 200 or 304 and If-Match with a 200 or 412 as appropriate.
- [x] [/forms/post](/forms/post) HTML form that submits to /post
- [x] [/gzip](/gzip) Returns gzip-encoded data.
- [x] [/headers](/headers) Returns request header dict.
- [x] [/hidden-basic-auth/:user/:passwd](/hidden-basic-auth/user/passwd) 404'd BasicAuth.
- [x] [/html](/html) Renders an HTML Page.
- [x] [/hostname](/hostname) Returns the name of the host serving the request.
- [x] [/image](/image) Returns page containing an image based on sent Accept header.
- [x] [/image/jpeg](/image/jpeg) Returns a JPEG image.
- [x] [/image/png](/image/png) Returns a PNG image.
- [x] [/image/svg](/image/svg) Returns a SVG image.
- [x] [/image/webp](/image/webp) Returns a WEBP image.
- [x] [/image/avif](/image/avif) Returns a AVIF image.
- [x] [/image/jxl](/image/jxl) Returns a JPEG-XL image.
- [x] [/ip](/ip) Returns Origin IP.
- [x] [/json](/json) Returns JSON.
- [x] [/links/:n](/links/10) Returns page containing n HTML links.
- [x] [/range/1024?duration=s&chunk_size=code](/range/1024?chunk_size=512) Streams n bytes, and allows specifying a Range header to select a subset of the data. Accepts a chunk_size and request duration parameter.
- [x] [/redirect-to?url=foo&status_code=307](/redirect-to?url=/&status_code=307) 307 Redirects to the foo URL.
- [x] [/redirect-to?url=foo](/redirect-to?url=/) 302 Redirects to the foo URL.
- [x] [/redirect/:n](/redirect/3) 302 Redirects n times.
- [x] [/relative-redirect/:n](/relative-redirect/3) 302 Relative redirects n times.
- [x] [/response-headers?key=val](/response-headers?key=val) Returns given response headers.
- [x] [/robots.txt](/robots.txt) Returns some robots.txt rules.
- [x] [/sse](/sse?delay=1s&duration=3s&count=10) a stream of server-sent events.
- [x] [/status/:code](/status/200) Returns given HTTP Status code.
- [x] [/stream-bytes/:n](/stream-bytes/1024?seed=123&chunk_size=512) Streams n random bytes of binary data, accepts optional seed and chunk_size integer parameters.
- [x] [/stream/:n](/stream/10) Streams min(n, 100) lines.
- [x] [/unstable](/unstable?failure_rate=0.6) Fails half the time, accepts optional failure_rate float and seed integer parameters.
- [x] [/user-agent](/user-agent) Returns user-agent.
- [x] [/uuid](/uuid) Generates a UUIDv4 value.
- [x] [/websocket/echo](/websocket/echo?max_fragment_size=2048&max_message_size=10240) A WebSocket echo service.
- [x] [/websocket/chat](/websocket/chat) A WebSocket chat service.
- [x] [/xml](/xml) Returns some XML
- [x] [/zstd](/zstd) Returns zstd-encoded data.
