"""The package version, in exactly one place.

`pyproject.toml` reads it for the wheel's metadata, `ponk.__version__`
re-exports it, and the client's User-Agent is built from it. It lives in its
own module so `ponk.client` can import it without importing the package that
imports `ponk.client`.
"""

__version__ = "0.3.0"
