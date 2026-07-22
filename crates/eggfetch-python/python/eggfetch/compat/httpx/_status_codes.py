"""HTTPX-compatible status codes namespace for eggfetch."""


class _StatusCodeGroup:
    """Namespace for HTTP status codes."""

    def __getattr__(self, name):
        try:
            return object.__getattribute__(self, name)
        except AttributeError:
            raise AttributeError(
                f"module 'eggfetch.compat.httpx._status_codes' has no attribute {name!r}"
            )


codes = _StatusCodeGroup()

codes.CONTINUE = 100
codes.SWITCHING_PROTOCOLS = 101
codes.PROCESSING = 102
codes.EARLY_HINTS = 103
codes.OK = 200
codes.CREATED = 201
codes.ACCEPTED = 202
codes.NON_AUTHORITATIVE_INFORMATION = 203
codes.NO_CONTENT = 204
codes.RESET_CONTENT = 205
codes.PARTIAL_CONTENT = 206
codes.MULTI_STATUS = 207
codes.ALREADY_REPORTED = 208
codes.IM_USED = 226
codes.MULTIPLE_CHOICES = 300
codes.MOVED_PERMANENTLY = 301
codes.FOUND = 302
codes.SEE_OTHER = 303
codes.NOT_MODIFIED = 304
codes.USE_PROXY = 305
codes.RESERVED = 306
codes.TEMPORARY_REDIRECT = 307
codes.PERMANENT_REDIRECT = 308
codes.BAD_REQUEST = 400
codes.UNAUTHORIZED = 401
codes.PAYMENT_REQUIRED = 402
codes.FORBIDDEN = 403
codes.NOT_FOUND = 404
codes.METHOD_NOT_ALLOWED = 405
codes.NOT_ACCEPTABLE = 406
codes.PROXY_AUTHENTICATION_REQUIRED = 407
codes.REQUEST_TIMEOUT = 408
codes.CONFLICT = 409
codes.GONE = 410
codes.LENGTH_REQUIRED = 411
codes.PRECONDITION_FAILED = 412
codes.CONTENT_TOO_LARGE = 413
codes.URI_TOO_LONG = 414
codes.UNSUPPORTED_MEDIA_TYPE = 415
codes.RANGE_NOT_SATISFIABLE = 416
codes.EXPECTATION_FAILED = 417
codes.IM_A_TEAPOT = 418
codes.MISDIRECTED_REQUEST = 421
codes.UNPROCESSABLE_CONTENT = 422
codes.LOCKED = 423
codes.FAILED_DEPENDENCY = 424
codes.TOO_EARLY = 425
codes.UPGRADE_REQUIRED = 426
codes.PRECONDITION_REQUIRED = 428
codes.TOO_MANY_REQUESTS = 429
codes.REQUEST_HEADER_FIELDS_TOO_LARGE = 431
codes.UNAVAILABLE_FOR_LEGAL_REASONS = 451
codes.INTERNAL_SERVER_ERROR = 500
codes.NOT_IMPLEMENTED = 501
codes.BAD_GATEWAY = 502
codes.SERVICE_UNAVAILABLE = 503
codes.GATEWAY_TIMEOUT = 504
codes.HTTP_VERSION_NOT_SUPPORTED = 505
codes.VARIANT_ALSO_NEGOTIATES = 506
codes.INSUFFICIENT_STORAGE = 507
codes.LOOP_DETECTED = 508
codes.NOT_EXTENDED = 510
codes.NETWORK_AUTHENTICATION_REQUIRED = 511
