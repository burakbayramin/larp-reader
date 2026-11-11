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


class InputUnavailable(FccError):
    title = "cannot open input"

    @classmethod
    def from_os_error(cls, e: OSError) -> "InputUnavailable":
        """Describe an OSError without its file name (which would reveal the title)."""
        if isinstance(e, FileNotFoundError):
            return cls("input file not found")
        return cls(f"input file unreadable: {e.strerror}" if e.strerror else "input file unreadable")
