class FccError(Exception):
    """Base error; `title` is used as the mask headline, str(e) as the description."""

    title = "error"


class UnsupportedFormat(FccError):
    title = "unsupported input format"


class DrmProtected(FccError):
    title = "DRM-protected input"


class NoTextLayer(FccError):
    title = "no text layer found"


class EmptyBook(FccError):
    title = "empty input"


class NoInput(FccError):
    title = "no input files"
